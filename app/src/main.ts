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

    // Render timeline
    if (activities.length === 0) {
      timelineEl.innerHTML = '<div class="empty-state">No activities recorded for this date</div>';
    } else {
      timelineEl.innerHTML = activities
        .map((activity) => {
          const color = getAppColor(activity.process_name);
          return `
            <div class="timeline-item" style="--app-color: ${color}">
              <div class="timeline-time">
                ${formatTime(activity.start_time)} - ${formatTime(activity.end_time)}
              </div>
              <div class="timeline-content">
                <div class="timeline-app">${escapeHtml(activity.process_name)}</div>
                <div class="timeline-title">${escapeHtml(activity.window_title)}</div>
                <div class="timeline-duration">${formatDuration(activity.duration_seconds)}</div>
              </div>
            </div>
          `;
        })
        .join("");
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

window.addEventListener("DOMContentLoaded", async () => {
  const datePicker = document.getElementById("date-picker") as HTMLInputElement;
  const toggleButton = document.getElementById("toggle-tracking")!;

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

  // Auto-refresh every 30 seconds
  setInterval(async () => {
    if (datePicker.value === getToday()) {
      await loadActivities(datePicker.value);
    }
  }, 30000);
});
