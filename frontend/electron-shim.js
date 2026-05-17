/*
 * electron-shim.js  --  Electron -> Tauri compatibility layer for opennest-rs.
 *
 * Deepnest's renderer was written for Electron with `nodeIntegration`, so it
 * calls `require()` for Node/Electron modules directly in the page. Tauri
 * webviews have no `require`, so this shim installs a `window.require` that
 * returns Tauri-backed implementations of every module Deepnest pulls in.
 *
 * It MUST be the first <script> in index.html and background.html, before any
 * Deepnest code runs. `window.__TAURI__` (enabled via `withGlobalTauri`) is
 * injected by Tauri before page scripts, so it is available here.
 *
 * Where Electron exposed a synchronous Node API that Tauri only offers
 * asynchronously, the shim degrades gracefully rather than blocking:
 *   - settings are kept in localStorage (genuinely synchronous, persistent),
 *   - file reads/writes go through Rust commands (async; sync writes are
 *     fire-and-forget),
 *   - the deepnest.io conversion/export server calls are stubbed (they need a
 *     backend that is not part of this project).
 */
(function () {
  "use strict";

  var T = window.__TAURI__ || {};
  if (!window.__TAURI__) {
    console.warn("[shim] window.__TAURI__ missing - is withGlobalTauri enabled?");
  }
  var invoke = (T.core && T.core.invoke) || function () {
    return Promise.reject(new Error("Tauri invoke unavailable"));
  };
  var tauriEvent = T.event || {};
  var tauriDialog = T.dialog || {};

  /* ---------------------------------------------------------------- path -- */
  var pathMod = {
    sep: "/",
    join: function () {
      var parts = [];
      for (var i = 0; i < arguments.length; i++) {
        var a = String(arguments[i] || "").replace(/\\/g, "/");
        if (a) parts.push(a);
      }
      var joined = parts.join("/").replace(/\/+/g, "/");
      // collapse "x/./y" and "x/../y"
      var segs = joined.split("/");
      var out = [];
      for (var s = 0; s < segs.length; s++) {
        if (segs[s] === "." || segs[s] === "") {
          if (s === 0 && segs[s] === "") out.push("");
          continue;
        }
        if (segs[s] === ".." && out.length && out[out.length - 1] !== "..") {
          out.pop();
        } else {
          out.push(segs[s]);
        }
      }
      return out.join("/") || ".";
    },
    basename: function (p, ext) {
      p = String(p || "").replace(/\\/g, "/");
      var b = p.substring(p.lastIndexOf("/") + 1);
      if (ext && b.substring(b.length - ext.length) === ext) {
        b = b.substring(0, b.length - ext.length);
      }
      return b;
    },
    dirname: function (p) {
      p = String(p || "").replace(/\\/g, "/");
      var i = p.lastIndexOf("/");
      return i <= 0 ? "." : p.substring(0, i);
    },
    extname: function (p) {
      var b = pathMod.basename(p);
      var i = b.lastIndexOf(".");
      return i <= 0 ? "" : b.substring(i);
    },
    resolve: function () {
      return pathMod.join.apply(null, arguments);
    },
  };

  /* ----------------------------------------------------------------- url -- */
  var urlMod = {
    format: function (o) {
      if (typeof o === "string") return o;
      var protocol = o.protocol || "file:";
      var p = String(o.pathname || "").replace(/\\/g, "/");
      var slashes = o.slashes ? "//" : "";
      return protocol + slashes + p;
    },
    parse: function (u) {
      return { href: u, pathname: u };
    },
  };

  /* ------------------------------------------------------------------ fs -- */
  // Backed by the `read_file` / `write_file` Rust commands so that
  // dialog-picked paths anywhere on disk work without fs-plugin scoping.
  function notFound(p) {
    var e = new Error("ENOENT: no such file or directory, '" + p + "'");
    e.code = "ENOENT";
    return e;
  }
  var fsMod = {
    readFile: function (p, enc, cb) {
      if (typeof enc === "function") {
        cb = enc;
      }
      invoke("read_file", { path: String(p) }).then(
        function (data) {
          cb && cb(null, data);
        },
        function (err) {
          cb && cb(notFound(p + " (" + err + ")"));
        }
      );
    },
    readFileSync: function (p) {
      // Truly synchronous reads are impossible on a Tauri webview. Callers in
      // Deepnest guard this with existsSync(), which returns false below, so
      // this path is not normally reached.
      throw notFound(p);
    },
    writeFile: function (p, data, cb) {
      if (typeof cb !== "function") cb = function () {};
      invoke("write_file", { path: String(p), contents: String(data) }).then(
        function () {
          cb(null);
        },
        function (err) {
          cb(new Error(String(err)));
        }
      );
    },
    writeFileSync: function (p, data) {
      // Fire-and-forget: the write happens, just not before this returns.
      invoke("write_file", { path: String(p), contents: String(data) }).catch(
        function (err) {
          console.error("[shim] writeFileSync failed:", err);
        }
      );
    },
    existsSync: function () {
      // Returning false makes Deepnest skip its NFP-disk-cache warmup and
      // cleanup loops; the cache simply lives in memory for the session.
      return false;
    },
    readdirSync: function () {
      return [];
    },
    lstatSync: function () {
      return { isDirectory: function () { return false; } };
    },
    statSync: function () {
      return { isDirectory: function () { return false; } };
    },
    unlinkSync: function () {},
    mkdirSync: function () {},
    createReadStream: function (p) {
      // Used only to stream a file to the conversion server, which is stubbed.
      console.warn("[shim] fs.createReadStream is not supported:", p);
      return { path: p, on: function () { return this; }, pipe: function () {} };
    },
  };

  /* ------------------------------------------------------- electron IPC -- */
  // Electron routed renderer<->renderer messages through the main process.
  // Tauri's emit() broadcasts to every webview, so main-process routing is
  // unnecessary: the background window listens for what the main window emits
  // and vice-versa; each side ignores its own echo.
  var ipcRenderer = {
    on: function (channel, listener) {
      if (tauriEvent.listen) {
        tauriEvent.listen(channel, function (e) {
          listener({ sender: ipcRenderer }, e.payload);
        });
      }
      return ipcRenderer;
    },
    once: function (channel, listener) {
      if (tauriEvent.once) {
        tauriEvent.once(channel, function (e) {
          listener({ sender: ipcRenderer }, e.payload);
        });
      }
      return ipcRenderer;
    },
    send: function (channel, payload) {
      // The nesting computation is no longer a hidden worker window — it runs
      // natively in the `run_nest` Rust command. Intercept the renderer's
      // `background-start` and turn it into an invoke; deliver the result back
      // as `background-response` (and let Rust emit `background-progress`).
      if (channel === "background-start") {
        invoke("run_nest", { input: payload }).then(
          function (result) {
            if (tauriEvent.emit) tauriEvent.emit("background-response", result);
          },
          function (err) {
            console.error("[shim] run_nest failed:", err);
          }
        );
        return;
      }
      if (channel === "background-stop") {
        // Electron destroyed/recreated the worker window here; the native
        // run finishes on its own and the next start runs fresh.
        return;
      }
      if (tauriEvent.emit) tauriEvent.emit(channel, payload);
    },
    sendSync: function () {
      return null;
    },
    removeAllListeners: function () {
      return ipcRenderer;
    },
  };

  /* ------------------------------------------------------ electron.remote -- */
  function showOpenDialog(opts, cb) {
    opts = opts || {};
    var filters = (opts.filters || []).map(function (f) {
      return { name: f.name, extensions: f.extensions };
    });
    if (!tauriDialog.open) {
      cb && cb(undefined);
      return;
    }
    tauriDialog
      .open({ multiple: true, directory: false, filters: filters })
      .then(function (selection) {
        if (!selection) {
          cb && cb(undefined);
        } else {
          cb && cb(Array.isArray(selection) ? selection : [selection]);
        }
      })
      .catch(function (err) {
        console.error("[shim] showOpenDialog:", err);
        cb && cb(undefined);
      });
  }
  function showSaveDialog(opts, cb) {
    opts = opts || {};
    if (!tauriDialog.save) {
      cb && cb(undefined);
      return;
    }
    tauriDialog
      .save({ title: opts.title, filters: opts.filters })
      .then(function (p) {
        cb && cb(p || undefined);
      })
      .catch(function (err) {
        console.error("[shim] showSaveDialog:", err);
        cb && cb(undefined);
      });
  }
  // electron.remote.showOpenDialog historically also worked when called
  // without a callback (returning a promise); both forms are supported.
  function dialogShim(fn) {
    return function (opts, cb) {
      if (typeof cb === "function") {
        fn(opts, cb);
        return undefined;
      }
      return new Promise(function (resolve) {
        fn(opts, resolve);
      });
    };
  }
  function StubWindow() {
    this.loadURL = function () {};
    this.show = function () {};
    this.hide = function () {};
    this.close = function () {};
    this.on = function () {};
    this.once = function () {};
    this.webContents = { on: function () {}, send: function () {} };
  }
  var remote = {
    dialog: {
      showOpenDialog: dialogShim(showOpenDialog),
      showSaveDialog: dialogShim(showSaveDialog),
    },
    BrowserWindow: StubWindow,
    getCurrentWindow: function () {
      return (T.window && T.window.getCurrentWindow && T.window.getCurrentWindow()) || {};
    },
    app: {
      getName: function () { return "opennest-rs"; },
      getVersion: function () { return pkg.version; },
      quit: function () {},
    },
    require: function (name) {
      // electron.remote.require loaded a module in the main process; the only
      // user is electron-window-manager, which we stub.
      console.warn("[shim] remote.require stubbed:", name);
      return windowManagerStub;
    },
    process: { platform: navigator.platform },
  };
  var windowManagerStub = {
    setDefaultSetupAndShow: function () {},
    sharedData: { fetch: function () {}, set: function () {} },
    createNew: function () { return new StubWindow(); },
    get: function () { return null; },
    open: function () { return new StubWindow(); },
  };

  /* ----------------------------------------------------- electron-settings -- */
  // electron-settings v4 exposed sync (getSync/setSync) and async (get/set)
  // forms. localStorage gives us a genuinely synchronous, persistent store.
  var SETTINGS_KEY = "opennest.settings";
  function loadSettings() {
    try {
      return JSON.parse(window.localStorage.getItem(SETTINGS_KEY)) || {};
    } catch (e) {
      return {};
    }
  }
  function saveSettings(obj) {
    try {
      window.localStorage.setItem(SETTINGS_KEY, JSON.stringify(obj));
    } catch (e) {
      console.error("[shim] settings save failed:", e);
    }
  }
  // Defaults registered via settings.defaults(); kept in memory like
  // electron-settings did, used by reset/apply.
  var settingsDefaults = {};
  var settings = {
    defaults: function (obj) {
      settingsDefaults = obj || {};
      return settings;
    },
    applyDefaultsSync: function () {
      var all = loadSettings();
      for (var k in settingsDefaults) {
        if (!Object.prototype.hasOwnProperty.call(all, k)) {
          all[k] = settingsDefaults[k];
        }
      }
      saveSettings(all);
    },
    applyDefaults: function () {
      settings.applyDefaultsSync();
      return Promise.resolve();
    },
    resetToDefaultsSync: function () {
      saveSettings(JSON.parse(JSON.stringify(settingsDefaults)));
    },
    resetToDefaults: function () {
      settings.resetToDefaultsSync();
      return Promise.resolve();
    },
    getSync: function (key) {
      var all = loadSettings();
      return key === undefined ? all : all[key];
    },
    setSync: function (key, val) {
      if (key !== undefined && typeof key === "object") {
        saveSettings(key);
        return;
      }
      var all = loadSettings();
      all[key] = val;
      saveSettings(all);
    },
    hasSync: function (key) {
      return Object.prototype.hasOwnProperty.call(loadSettings(), key);
    },
    deleteSync: function (key) {
      var all = loadSettings();
      delete all[key];
      saveSettings(all);
    },
    get: function (key) {
      return Promise.resolve(settings.getSync(key));
    },
    set: function (key, val) {
      settings.setSync(key, val);
      return Promise.resolve();
    },
    has: function (key) {
      return Promise.resolve(settings.hasSync(key));
    },
    unset: function (key) {
      settings.deleteSync(key);
      return Promise.resolve();
    },
    getAll: function () {
      return Promise.resolve(loadSettings());
    },
    file: function () {
      return SETTINGS_KEY;
    },
  };

  /* -------------------------------------------------------------- request -- */
  // The npm `request` library streamed multipart uploads to deepnest.io's
  // conversion/export backend. That backend is not part of this project, so
  // these calls fail cleanly and the UI shows its "server unavailable" message.
  function request(opts, cb) {
    var err = new Error("conversion/export server is not available in this build");
    if (typeof cb === "function") setTimeout(function () { cb(err); }, 0);
    return makeReq(cb);
  }
  function makeReq(cb) {
    var req = {
      form: function () {
        return { append: function () {}, getHeaders: function () { return {}; } };
      },
      on: function () { return req; },
      pipe: function () { return req; },
      end: function () {},
      write: function () {},
    };
    return req;
  }
  request.post = function (opts, cb) {
    return request(opts, cb);
  };
  request.get = function (opts, cb) {
    return request(opts, cb);
  };

  var httpMod = {
    request: function () { return makeReq(); },
    get: function () { return makeReq(); },
  };

  /* ----------------------------------------------------------- filequeue -- */
  function FileQueue() {}
  FileQueue.prototype.writeFile = function (p, data, cbOrEnc, cb) {
    var done = typeof cbOrEnc === "function" ? cbOrEnc : cb;
    fsMod.writeFile(p, data, done || function () {});
  };
  FileQueue.prototype.readFile = function (p, enc, cb) {
    fsMod.readFile(p, enc, cb);
  };

  /* --------------------------------------------------------------- addon -- */
  // Deepnest's renderer required a native C++ NFP addon. All nesting geometry
  // (NFP + the whole placement algorithm) now runs in the Rust `run_nest`
  // command, so this is just a guard for any lingering reference — the worker
  // window and background.js are no longer loaded.
  var addon = {
    calculateNFP: function () {
      throw new Error("NFP now runs in the Rust backend (run_nest), not the page");
    },
    calculateNFPBatch: function () {
      throw new Error("NFP now runs in the Rust backend (run_nest), not the page");
    },
  };

  /* --------------------------------------------------------- package.json -- */
  var pkg = { version: "1.0.5", name: "opennest-rs" };

  /* ------------------------------------------------------- module registry -- */
  var modules = {
    electron: { ipcRenderer: ipcRenderer, remote: remote, shell: { openExternal: function (u) { if (T.opener && T.opener.openUrl) T.opener.openUrl(u); } } },
    "graceful-fs": fsMod,
    fs: fsMod,
    path: pathMod,
    url: urlMod,
    request: request,
    http: httpMod,
    https: httpMod,
    "electron-settings": settings,
    "electron-config": settings,
    filequeue: FileQueue,
    "electron-window-manager": windowManagerStub,
    os: { cpus: function () { return new Array(navigator.hardwareConcurrency || 4); }, platform: function () { return "browser"; } },
  };

  window.require = function (name) {
    if (Object.prototype.hasOwnProperty.call(modules, name)) {
      return modules[name];
    }
    // the native NFP addon, required by relative path from background.js
    if (/minkowski|addon/.test(name)) {
      return addon;
    }
    if (/package\.json$/.test(name)) {
      return pkg;
    }
    console.warn("[shim] require('" + name + "') is not implemented; returning {}");
    return {};
  };

  // Globals Deepnest expects from Node integration.
  window.__dirname = ".";
  window.__filename = "index.html";
  if (!window.global) window.global = window;
  if (!window.process) {
    window.process = {
      platform: "browser",
      env: {},
      argv: [],
      version: "",
      versions: {},
      nextTick: function (fn) { setTimeout(fn, 0); },
      on: function () {},
      cwd: function () { return "."; },
    };
  }

  console.log("[shim] Electron compatibility layer installed");
})();
