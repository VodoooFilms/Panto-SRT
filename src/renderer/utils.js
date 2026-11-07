'use strict';

// Shared formatting helpers for the three Panto Studio tabs.
(function () {
  function formatBytes(bytes) {
    const value = Number(bytes) || 0;
    if (value <= 0) return '0 B';
    if (value < 1024) return `${value} B`;
    const units = ['KB', 'MB', 'GB'];
    let index = -1;
    let size = value;
    do {
      size /= 1024;
      index += 1;
    } while (size >= 1024 && index < units.length - 1);
    return `${size.toFixed(size >= 100 ? 0 : size >= 10 ? 1 : 2)} ${units[index]}`;
  }

  function formatBytesMb(bytes) {
    if (!Number.isFinite(bytes) || bytes <= 0) return '0 MB';
    const megabytes = bytes / (1024 ** 2);
    return `${megabytes < 1 ? megabytes.toFixed(2) : megabytes.toFixed(1)} MB`;
  }

  function formatDuration(totalSeconds) {
    const total = Math.round(Number(totalSeconds) || 0);
    if (!total) return '';
    const hours = Math.floor(total / 3600);
    const minutes = Math.floor((total % 3600) / 60);
    const seconds = total % 60;
    if (hours) return `${hours}:${String(minutes).padStart(2, '0')}:${String(seconds).padStart(2, '0')}`;
    return `${minutes}:${String(seconds).padStart(2, '0')}`;
  }

  function secondsToStamp(seconds, includeMillis = true) {
    const total = Math.max(0, Math.round(seconds * 1000));
    const hours = Math.floor(total / 3600000);
    const minutes = Math.floor((total % 3600000) / 60000);
    const secs = Math.floor((total % 60000) / 1000);
    const millis = total % 1000;
    const pad = (n, w = 2) => String(n).padStart(w, '0');
    const body = `${pad(hours)}:${pad(minutes)}:${pad(secs)}`;
    return includeMillis ? `${body},${pad(millis, 3)}` : body;
  }

  window.PantoUtils = { formatBytes, formatBytesMb, formatDuration, secondsToStamp };
})();
