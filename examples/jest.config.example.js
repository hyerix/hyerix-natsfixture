// jest.config.js — wire hyerix-natsfixture into Jest's global setup.
//
// The fixture spawns once per test run, exposes NATS_URL to all test files,
// and is killed in globalTeardown.
//
// Alternative: just wrap your `jest` invocation with `hyerix-natsfixture exec`:
//
//   hyerix-natsfixture exec --manifest fixture.yaml -- jest
//
// That's simpler if you don't need Jest globals — pick whichever fits.

module.exports = {
  globalSetup: "./jest.setup.js",
  globalTeardown: "./jest.teardown.js",
  // ... your other Jest config ...
};

// --- jest.setup.js ---
//
// const { spawn } = require("node:child_process");
// const fs = require("node:fs");
// const path = require("node:path");
//
// module.exports = async () => {
//   const urlFile = path.join(__dirname, ".nats-url");
//   const pidFile = path.join(__dirname, ".nats-pid");
//
//   const child = spawn("hyerix-natsfixture", [
//     "spawn",
//     "--manifest", "fixture.yaml",
//     "--url-file", urlFile,
//     "--pid-file", pidFile,
//   ], { stdio: "inherit" });
//
//   // Wait for the URL file to appear (fixture writes it after NATS_FIXTURE_READY).
//   const deadline = Date.now() + 5000;
//   while (!fs.existsSync(urlFile)) {
//     if (Date.now() > deadline) throw new Error("fixture failed to start");
//     await new Promise((r) => setTimeout(r, 50));
//   }
//   process.env.NATS_URL = fs.readFileSync(urlFile, "utf8").trim();
// };
//
// --- jest.teardown.js ---
//
// const { spawnSync } = require("node:child_process");
// const path = require("node:path");
//
// module.exports = async () => {
//   spawnSync("hyerix-natsfixture", [
//     "kill",
//     "--pid-file", path.join(__dirname, ".nats-pid"),
//   ], { stdio: "inherit" });
// };
