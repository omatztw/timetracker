import { invoke } from "@tauri-apps/api/core";

interface ActivityRecord {
  id: number;
  process_name: string;
  window_title: string;
  domain: string | null;
  start_time: string;
  end_time: string;
  duration_seconds: number;
}

interface AppSummary {
  process_name: string;
  total_seconds: number;
  percentage: number;
}

interface DomainSummary {
  domain: string;
  total_seconds: number;
  percentage: number;
}

interface SyncResult {
  success: boolean;
  message: string;
  external_id: string | null;
}

// Store detected ticket IDs for each activity
const activityTickets: Map<number, Array<[string, string]>> = new Map();

// Color palette for apps
const APP_COLORS: Record<string, string> = {};
const COLOR_PALETTE = [
  "#4285f4", "#ea4335", "#fbbc04", "#34a853", "#ff6d01",
  "#46bdc6", "#7baaf7", "#f07b72", "#fcd04f", "#5bb974",
  "#ff9e80", "#81d4fa", "#a5d6a7", "#ffe082", "#f48fb1",
  "#b39ddb", "#80deea", "#c5e1a5", "#ffcc80", "#ef9a9a",
];
let colorIndex = 0;

function getAppColor(appName: string): string {
  if (!APP_COLORS[appName]) {
    APP_COLORS[appName] = COLOR_PALETTE[colorIndex % COLOR_PALETTE.length];
    colorIndex++;
  }
  return APP_COLORS[appName];
}

function formatDuration(seconds: number): string {
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const secs = seconds % 60;

  if (hours > 0) {
    return `${hours}h ${minutes}m`;
  } else if (minutes > 0) {
    return `${minutes}m ${secs}s`;
  } else {
    return `${secs}s`;
  }
}

function formatTime(isoString: string): string {
  const date = new Date(isoString);
  return date.toLocaleTimeString("ja-JP", {
    hour: "2-digit",
    minute: "2-digit",
  });
}

function getToday(): string {
  const today = new Date();
  return today.toISOString().split("T")[0];
}

async function loadActivities(date: string): Promise<void> {
  const timelineEl = document.getElementById("timeline")!;
  const summaryEl = document.getElementById("summary")!;
  const domainSummaryEl = document.getElementById("domain-summary")!;
  const statusEl = document.getElementById("status")!;

  try {
    statusEl.textContent = "Loading...";

    const [activities, summary, domainSummary] = await Promise.all([
      invoke<ActivityRecord[]>("get_activities", { date }),
      invoke<AppSummary[]>("get_app_summary", { date }),
      invoke<DomainSummary[]>("get_domain_summary", { date }),
    ]);

    // Get active plugins
    const plugins = await invoke<string[]>("get_plugins");
    const hasPlugins = plugins.length > 0;

    // Render timeline
    if (activities.length === 0) {
      timelineEl.innerHTML = '<div class="empty-state">No activities recorded for this date</div>';
    } else {
      // Extract ticket IDs for all activities if plugins are available
      if (hasPlugins) {
        for (const activity of activities) {
          try {
            const tickets = await invoke<Array<[string, string]>>("extract_ticket_ids", {
              activityId: activity.id,
            });
            if (tickets.length > 0) {
              activityTickets.set(activity.id, tickets);
            }
          } catch {
            // Ignore extraction errors
          }
        }
      }

      timelineEl.innerHTML = activities
        .map((activity) => {
          const color = getAppColor(activity.process_name);
          const tickets = activityTickets.get(activity.id) || [];
          const ticketBadges = tickets
            .map(([plugin, ticketId]) => `<span class="ticket-badge" title="${escapeHtml(plugin)}">#${escapeHtml(ticketId)}</span>`)
            .join("");
          const syncButtons = tickets
            .map(
              ([plugin, ticketId]) =>
                `<button class="btn-sync" data-activity-id="${activity.id}" data-plugin="${escapeHtml(plugin)}" data-ticket="${escapeHtml(ticketId)}">Sync to ${escapeHtml(plugin)} #${escapeHtml(ticketId)}</button>`
            )
            .join("");

          return `
            <div class="timeline-item" style="--app-color: ${color}">
              <div class="timeline-time">
                ${formatTime(activity.start_time)} - ${formatTime(activity.end_time)}
              </div>
              <div class="timeline-content">
                <div class="timeline-app">${escapeHtml(activity.process_name)}${ticketBadges}</div>
                <div class="timeline-title">${escapeHtml(activity.window_title)}</div>
                <div class="timeline-duration">${formatDuration(activity.duration_seconds)}</div>
                ${syncButtons ? `<div class="timeline-actions">${syncButtons}</div>` : ""}
              </div>
            </div>
          `;
        })
        .join("");

      // Add click handlers for sync buttons
      timelineEl.querySelectorAll(".btn-sync").forEach((btn) => {
        btn.addEventListener("click", handleSyncClick);
      });
    }

    // Render summary
    if (summary.length === 0) {
      summaryEl.innerHTML = '<div class="empty-state">No data available</div>';
    } else {
      summaryEl.innerHTML = summary
        .map((app) => {
          const color = getAppColor(app.process_name);
          return `
            <div class="summary-item">
              <div class="summary-bar" style="width: ${app.percentage}%; background-color: ${color}"></div>
              <div class="summary-info">
                <span class="summary-app">${escapeHtml(app.process_name)}</span>
                <span class="summary-stats">
                  ${formatDuration(app.total_seconds)} (${app.percentage.toFixed(1)}%)
                </span>
              </div>
            </div>
          `;
        })
        .join("");
    }

    // Render domain summary
    if (domainSummary.length === 0) {
      domainSummaryEl.innerHTML = '<div class="empty-state">No browser activity</div>';
    } else {
      domainSummaryEl.innerHTML = domainSummary
        .map((item) => {
          const color = getAppColor(item.domain);
          return `
            <div class="summary-item">
              <div class="summary-bar" style="width: ${item.percentage}%; background-color: ${color}"></div>
              <div class="summary-info">
                <span class="summary-app">${escapeHtml(item.domain)}</span>
                <span class="summary-stats">
                  ${formatDuration(item.total_seconds)} (${item.percentage.toFixed(1)}%)
                </span>
              </div>
            </div>
          `;
        })
        .join("");
    }

    const totalSeconds = summary.reduce((acc, app) => acc + app.total_seconds, 0);
    statusEl.textContent = `Total tracked: ${formatDuration(totalSeconds)}`;
  } catch (error) {
    statusEl.textContent = `Error: ${error}`;
    console.error("Failed to load activities:", error);
  }
}

function escapeHtml(text: string): string {
  const div = document.createElement("div");
  div.textContent = text;
  return div.innerHTML;
}

async function updateTrackingButton(): Promise<void> {
  const button = document.getElementById("toggle-tracking")!;
  const isTracking = await invoke<boolean>("is_tracking");
  button.textContent = `Tracking: ${isTracking ? "ON" : "OFF"}`;
  button.classList.toggle("tracking-on", isTracking);
  button.classList.toggle("tracking-off", !isTracking);
}

async function toggleTracking(): Promise<void> {
  const isTracking = await invoke<boolean>("is_tracking");
  if (isTracking) {
    await invoke("stop_tracking");
  } else {
    await invoke("start_tracking");
  }
  await updateTrackingButton();
}

// ========== Plugin Integration Functions ==========

async function handleSyncClick(event: Event): Promise<void> {
  const btn = event.target as HTMLButtonElement;
  const activityId = parseInt(btn.dataset.activityId || "0", 10);
  const plugin = btn.dataset.plugin || "";
  const ticketId = btn.dataset.ticket || "";

  if (!activityId || !plugin || !ticketId) return;

  btn.disabled = true;
  btn.textContent = "Syncing...";

  try {
    const result = await invoke<SyncResult>("sync_time_entry", {
      pluginName: plugin,
      activityId,
      ticketId,
    });

    if (result.success) {
      btn.textContent = "Synced!";
      btn.style.backgroundColor = "var(--success)";
    } else {
      btn.textContent = "Failed";
      btn.style.backgroundColor = "var(--warning)";
      console.error("Sync failed:", result.message);
    }
  } catch (error) {
    btn.textContent = "Error";
    btn.style.backgroundColor = "var(--warning)";
    console.error("Sync error:", error);
  }

  // Re-enable after 3 seconds
  setTimeout(() => {
    btn.disabled = false;
    btn.textContent = `Sync to ${plugin} #${ticketId}`;
    btn.style.backgroundColor = "";
  }, 3000);
}

async function openIntegrationsModal(): Promise<void> {
  const modal = document.getElementById("integrations-modal")!;
  const configPathEl = document.getElementById("config-path")!;
  const pluginsListEl = document.getElementById("plugins-list")!;

  // Get config path
  const configPath = await invoke<string>("get_plugin_config_path");
  configPathEl.textContent = configPath;

  // Get active plugins
  await refreshPluginsList(pluginsListEl);

  modal.classList.remove("hidden");
}

async function refreshPluginsList(pluginsListEl: HTMLElement): Promise<void> {
  const plugins = await invoke<string[]>("get_plugins");

  if (plugins.length === 0) {
    pluginsListEl.innerHTML = '<p class="empty-state-small">No plugins configured</p>';
  } else {
    pluginsListEl.innerHTML = plugins
      .map(
        (name) => `
        <div class="plugin-item">
          <span class="plugin-name">${escapeHtml(name)}</span>
          <span class="plugin-status">Active</span>
        </div>
      `
      )
      .join("");
  }
}

function closeIntegrationsModal(): void {
  const modal = document.getElementById("integrations-modal")!;
  modal.classList.add("hidden");
}

async function createSampleConfig(): Promise<void> {
  try {
    const path = await invoke<string>("create_sample_plugin_config");
    alert(`Sample config created at:\n${path}\n\nEdit this file to configure your integrations.`);
  } catch (error) {
    alert(`Failed to create sample config: ${error}`);
  }
}

async function reloadPlugins(): Promise<void> {
  const pluginsListEl = document.getElementById("plugins-list")!;
  try {
    await invoke("reload_plugins");
    await refreshPluginsList(pluginsListEl);
    alert("Plugins reloaded successfully!");
  } catch (error) {
    alert(`Failed to reload plugins: ${error}`);
  }
}

window.addEventListener("DOMContentLoaded", async () => {
  const datePicker = document.getElementById("date-picker") as HTMLInputElement;
  const toggleButton = document.getElementById("toggle-tracking")!;
  const integrationsBtn = document.getElementById("integrations-btn")!;
  const closeModalBtn = document.getElementById("close-modal")!;
  const createSampleBtn = document.getElementById("create-sample-config")!;
  const reloadPluginsBtn = document.getElementById("reload-plugins")!;
  const modal = document.getElementById("integrations-modal")!;

  // Set today's date
  datePicker.value = getToday();

  // Load initial data
  await loadActivities(datePicker.value);
  await updateTrackingButton();

  // Event listeners
  datePicker.addEventListener("change", () => {
    loadActivities(datePicker.value);
  });

  toggleButton.addEventListener("click", toggleTracking);

  // Integrations modal events
  integrationsBtn.addEventListener("click", openIntegrationsModal);
  closeModalBtn.addEventListener("click", closeIntegrationsModal);
  createSampleBtn.addEventListener("click", createSampleConfig);
  reloadPluginsBtn.addEventListener("click", reloadPlugins);

  // Close modal when clicking outside
  modal.addEventListener("click", (e) => {
    if (e.target === modal) {
      closeIntegrationsModal();
    }
  });

  // Auto-refresh every 30 seconds
  setInterval(async () => {
    if (datePicker.value === getToday()) {
      await loadActivities(datePicker.value);
    }
  }, 30000);
});
