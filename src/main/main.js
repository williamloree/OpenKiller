/**
 * Open Killer - Application Electron
 * Fichier principal pour la gestion de la fenêtre et des processus système
 */

const { app, BrowserWindow, ipcMain } = require("electron");
const path = require("path");
const { exec } = require("child_process");
const util = require("util");
const execPromise = util.promisify(exec);

let mainWindow;

/**
 * Crée la fenêtre principale de l'application
 */
function createWindow() {
  mainWindow = new BrowserWindow({
    width: 1200,
    height: 800,
    resizable: false,
    webPreferences: {
      nodeIntegration: false,
      contextIsolation: true,
      preload: path.join(__dirname, "..", "preload", "preload.js"),
    },
    backgroundColor: "#1a1a1a",
    icon: path.join(__dirname, "..", "..", "assets", "icon.png"),
    autoHideMenuBar: true,
    show: false,
  });

  mainWindow.loadFile(path.join(__dirname, "..", "renderer", "index.html"));

  // Afficher la fenêtre une fois prête pour éviter le flash
  mainWindow.once("ready-to-show", () => {
    mainWindow.show();
  });

  // Ouvrir DevTools en mode développement
  if (process.argv.includes("--dev")) {
    mainWindow.webContents.openDevTools();
  }
}

/**
 * Récupère la liste des ports ouverts sur le système
 * Compatible Windows, macOS et Linux
 */
async function getOpenPorts() {
  const platform = process.platform;
  let command;

  try {
    if (platform === "win32") {
      // Commande pour Windows
      command = "netstat -ano | findstr LISTENING";
    } else if (platform === "darwin") {
      // Commande pour macOS
      command = "lsof -iTCP -sTCP:LISTEN -n -P";
    } else {
      // Commande pour Linux
      command = "ss -tlnp || netstat -tlnp";
    }

    const { stdout } = await execPromise(command);
    const ports = parsePortData(stdout, platform);

    await resolveProcessDetails(ports);

    return ports;
  } catch (error) {
    console.error("Erreur lors de la récupération des ports:", error);
    return [];
  }
}

/**
 * Enrichit chaque port avec le nom réel (Windows), la mémoire utilisée et le chemin de l'exécutable
 * @param {Array} ports - Liste des ports à mettre à jour
 */
async function resolveProcessDetails(ports) {
  const platform = process.platform;
  const pids = [...new Set(ports.map((p) => p.pid))];
  const details = await Promise.all(pids.map((pid) => getProcessDetails(pid)));
  const detailByPid = new Map(pids.map((pid, i) => [pid, details[i]]));

  ports.forEach((port) => {
    const detail = detailByPid.get(port.pid);
    if (!detail) return;

    if (platform === "win32" && detail.name) {
      port.processName = detail.name;
    }
    port.memoryMB = detail.memoryMB;
    port.exePath = detail.path;
  });
}

/**
 * Parse les données de ports selon le système d'exploitation
 * @param {string} data - Sortie brute de la commande système
 * @param {string} platform - Plateforme système (win32, darwin, linux)
 * @returns {Array} - Liste des ports avec leurs informations
 */
function parsePortData(data, platform) {
  const lines = data.split("\n").filter((line) => line.trim());
  const ports = [];
  const seenPorts = new Set();

  lines.forEach((line) => {
    try {
      let port, pid, protocol, address, processName;

      if (platform === "win32") {
        // Parse pour Windows: TCP 0.0.0.0:3000 0.0.0.0:0 LISTENING 1234
        const parts = line.trim().split(/\s+/);
        if (parts.length >= 5) {
          protocol = parts[0];
          const addressParts = parts[1].split(":");
          port = addressParts[addressParts.length - 1];
          address = parts[1];
          pid = parts[parts.length - 1];
          processName = "Processus système";
        }
      } else if (platform === "darwin") {
        // Parse pour macOS: node 1234 user 20u IPv4 0x... 0t0 TCP *:3000 (LISTEN)
        const parts = line.trim().split(/\s+/);
        if (parts.length >= 9) {
          processName = parts[0];
          pid = parts[1];
          const portInfo = parts[8].split(":");
          port = portInfo[portInfo.length - 1];
          protocol = parts[7];
          address = parts[8];
        }
      } else {
        // Parse pour Linux (ss ou netstat)
        if (line.includes("LISTEN")) {
          const parts = line.trim().split(/\s+/);
          if (parts.length >= 5) {
            protocol = parts[0];
            const addressParts = parts[4].split(":");
            port = addressParts[addressParts.length - 1];
            address = parts[4];

            // Extraire PID et nom du processus de la dernière colonne
            const lastCol = parts[parts.length - 1];
            const pidMatch = lastCol.match(/pid=(\d+)/);
            pid = pidMatch ? pidMatch[1] : "N/A";

            const nameMatch = lastCol.match(/users:\(\("([^"]+)"/);
            processName = nameMatch ? nameMatch[1] : "Processus système";
          }
        }
      }

      // Ajouter le port s'il est valide et non dupliqué
      if (port && !seenPorts.has(port)) {
        seenPorts.add(port);
        ports.push({
          port: parseInt(port) || port,
          pid: pid || "N/A",
          protocol: protocol || "TCP",
          address: address || "N/A",
          processName: processName || "Inconnu",
          timestamp: Date.now(),
        });
      }
    } catch (error) {
      // Ignorer les lignes mal formées
      console.warn("Ligne ignorée:", line);
    }
  });

  return ports.sort((a, b) => {
    const portA = typeof a.port === "number" ? a.port : parseInt(a.port) || 0;
    const portB = typeof b.port === "number" ? b.port : parseInt(b.port) || 0;
    return portA - portB;
  });
}

/**
 * Tue un processus par son PID
 * @param {string|number} pid - ID du processus à terminer
 */
async function killProcess(pid) {
  if (!/^\d+$/.test(String(pid))) {
    return { success: false, message: "PID invalide" };
  }

  const platform = process.platform;
  let command;

  try {
    if (platform === "win32") {
      command = `taskkill /F /PID ${pid}`;
    } else {
      command = `kill -9 ${pid}`;
    }

    await execPromise(command);
    return { success: true, message: `Processus ${pid} terminé avec succès` };
  } catch (error) {
    console.error(`Erreur lors de la terminaison du processus ${pid}:`, error);
    return {
      success: false,
      message: `Échec de la terminaison du processus: ${error.message}`,
    };
  }
}

/**
 * Récupère nom, mémoire (Mo) et chemin de l'exécutable pour un PID donné
 * @param {string|number} pid - ID du processus
 * @returns {Promise<{name: string|null, memoryMB: number|null, path: string}>}
 */
async function getProcessDetails(pid) {
  const empty = { name: null, memoryMB: null, path: "N/A" };

  if (!/^\d+$/.test(String(pid))) {
    return empty;
  }

  const platform = process.platform;

  try {
    if (platform === "win32") {
      const command = `powershell -NoProfile -NonInteractive -Command "Get-Process -Id ${pid} | Select-Object ProcessName,Path,WorkingSet64 | ConvertTo-Json -Compress"`;
      const { stdout } = await execPromise(command);
      const data = JSON.parse(stdout);

      return {
        name: data.ProcessName || null,
        memoryMB: data.WorkingSet64
          ? Math.round(data.WorkingSet64 / 1024 / 1024)
          : null,
        path: data.Path || "N/A",
      };
    }

    const { stdout } = await execPromise(`ps -p ${pid} -o rss=,args=`);
    const match = stdout.trim().match(/^(\d+)\s+(.*)$/);
    if (!match) return empty;

    const [, rss, args] = match;
    let processPath = args.split(" ")[0] || "N/A";

    if (platform === "linux") {
      try {
        const { stdout: exe } = await execPromise(`readlink -f /proc/${pid}/exe`);
        if (exe.trim()) processPath = exe.trim();
      } catch (error) {
        // Garder le chemin déduit de la commande si /proc indisponible
      }
    }

    return {
      name: null,
      memoryMB: Math.round(parseInt(rss, 10) / 1024),
      path: processPath,
    };
  } catch (error) {
    return empty;
  }
}

/**
 * Récupère l'état de la RAM système : total, utilisée et estimation de RAM
 * libérable (cache disque / fichiers reclaimable par l'OS)
 * @returns {Promise<{totalGB: number, usedGB: number, freeableGB: number}>}
 */
async function getMemoryInfo() {
  const empty = { totalGB: 0, usedGB: 0, freeableGB: 0 };
  const platform = process.platform;

  try {
    if (platform === "win32") {
      const command =
        'powershell -NoProfile -NonInteractive -Command "$os = Get-CimInstance Win32_OperatingSystem; $perf = Get-CimInstance Win32_PerfFormattedData_PerfOS_Memory; [PSCustomObject]@{Total=($os.TotalVisibleMemorySize*1024); Available=$perf.AvailableBytes; Cache=$perf.CacheBytes} | ConvertTo-Json -Compress"';
      const { stdout } = await execPromise(command);
      const data = JSON.parse(stdout);

      const totalGB = data.Total / 1073741824;
      const usedGB = totalGB - data.Available / 1073741824;
      const freeableGB = Math.min(data.Cache / 1073741824, usedGB);

      return { totalGB, usedGB: Math.max(usedGB, 0), freeableGB: Math.max(freeableGB, 0) };
    }

    if (platform === "darwin") {
      const { stdout: memsizeOut } = await execPromise("sysctl -n hw.memsize");
      const totalBytes = parseInt(memsizeOut.trim(), 10);

      const { stdout: vmStatOut } = await execPromise("vm_stat");
      const pageSizeMatch = vmStatOut.match(/page size of (\d+) bytes/);
      const pageSize = pageSizeMatch ? parseInt(pageSizeMatch[1], 10) : 4096;

      const getPages = (label) => {
        const m = vmStatOut.match(new RegExp(`${label}:\\s+(\\d+)`));
        return m ? parseInt(m[1], 10) : 0;
      };

      const freeBytes = getPages("Pages free") * pageSize;
      const freeableBytes = getPages("Pages inactive") * pageSize;

      const totalGB = totalBytes / 1073741824;
      const usedGB = totalGB - freeBytes / 1073741824;
      const freeableGB = freeableBytes / 1073741824;

      return { totalGB, usedGB: Math.max(usedGB, 0), freeableGB: Math.max(freeableGB, 0) };
    }

    // Linux
    const { stdout } = await execPromise("cat /proc/meminfo");
    const getKB = (label) => {
      const m = stdout.match(new RegExp(`${label}:\\s+(\\d+)`));
      return m ? parseInt(m[1], 10) * 1024 : 0;
    };

    const totalBytes = getKB("MemTotal");
    const availableBytes = getKB("MemAvailable");
    const freeableBytes = getKB("Cached") + getKB("Buffers");

    const totalGB = totalBytes / 1073741824;
    const usedGB = totalGB - availableBytes / 1073741824;
    const freeableGB = freeableBytes / 1073741824;

    return { totalGB, usedGB: Math.max(usedGB, 0), freeableGB: Math.max(freeableGB, 0) };
  } catch (error) {
    console.error("Erreur lors de la récupération de l'état de la RAM:", error);
    return empty;
  }
}

/**
 * Libère la RAM inutilisée par les applications (working set trimming)
 * Windows: élévation UAC ponctuelle pour vider le working set de tous les processus
 * macOS: commande `purge` (cache mémoire système)
 * Linux: vidage du page cache via pkexec (demande d'authentification graphique)
 */
async function cleanMemory() {
  const platform = process.platform;

  try {
    if (platform === "win32") {
      return await cleanMemoryWindows();
    } else if (platform === "darwin") {
      await execPromise("purge");
      return { success: true, message: "RAM inutilisée libérée" };
    } else {
      await execPromise('pkexec sh -c "sync; echo 3 > /proc/sys/vm/drop_caches"');
      return { success: true, message: "RAM inutilisée libérée" };
    }
  } catch (error) {
    if (/cancel|denied|dismiss/i.test(error.message)) {
      return { success: false, message: "Opération annulée" };
    }
    return { success: false, message: `Échec: ${error.message}` };
  }
}

/**
 * Vide le working set de tous les processus accessibles via une commande
 * PowerShell élevée (UAC), pour libérer la mémoire physique qu'ils ne
 * réutilisent pas immédiatement (API documentée EmptyWorkingSet)
 */
async function cleanMemoryWindows() {
  const innerScript = `
try {
  $code = @'
using System;
using System.Runtime.InteropServices;
public class OpenKillerMem {
    [DllImport("psapi.dll")]
    public static extern bool EmptyWorkingSet(IntPtr hProcess);
}
'@
  Add-Type -TypeDefinition $code -Language CSharp -ErrorAction Stop
  Get-Process | ForEach-Object {
    try { [OpenKillerMem]::EmptyWorkingSet($_.Handle) | Out-Null } catch {}
  }
  exit 0
} catch {
  exit 1
}
`;
  const innerEncoded = Buffer.from(innerScript, "utf16le").toString("base64");

  const outerScript = `
try {
  $p = Start-Process powershell -Verb RunAs -WindowStyle Hidden -Wait -PassThru -ArgumentList '-NoProfile -WindowStyle Hidden -EncodedCommand ${innerEncoded}'
  exit $p.ExitCode
} catch {
  exit 1
}
`;
  const outerEncoded = Buffer.from(outerScript, "utf16le").toString("base64");

  try {
    await execPromise(
      `powershell -NoProfile -WindowStyle Hidden -EncodedCommand ${outerEncoded}`
    );
    return { success: true, message: "RAM inutilisée libérée" };
  } catch (error) {
    return { success: false, message: "Opération annulée ou refusée (élévation requise)" };
  }
}

// Gestionnaires d'événements IPC
ipcMain.handle("get-ports", async () => {
  return await getOpenPorts();
});

ipcMain.handle("clean-ram", async () => {
  return await cleanMemory();
});

ipcMain.handle("get-memory-info", async () => {
  return await getMemoryInfo();
});

ipcMain.handle("kill-process", async (event, pid) => {
  return await killProcess(pid);
});

ipcMain.handle("kill-processes", async (event, pids) => {
  if (!Array.isArray(pids)) return [];

  const results = await Promise.all(pids.map((pid) => killProcess(pid)));
  return results.map((result, i) => ({ pid: pids[i], ...result }));
});

// Événements de cycle de vie de l'application
app.whenReady().then(createWindow);

app.on("window-all-closed", () => {
  if (process.platform !== "darwin") {
    app.quit();
  }
});

app.on("activate", () => {
  if (BrowserWindow.getAllWindows().length === 0) {
    createWindow();
  }
});

// Gestion des erreurs non capturées
process.on("uncaughtException", (error) => {
  console.error("Erreur non capturée:", error);
});
