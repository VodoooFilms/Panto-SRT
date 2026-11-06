'use strict';

// Tauri bridge: exposes the same `window.pantoSRT` API the Electron preload
// used to provide, backed by Tauri commands and events.
(function () {
  const tauri = window.__TAURI__;
  const invoke = tauri.core.invoke;
  const listen = tauri.event.listen;

  let fileDropCallback = null;
  if (tauri.window && tauri.window.getCurrentWindow) {
    tauri.window.getCurrentWindow().onDragDropEvent((event) => {
      if (event.payload && event.payload.type === 'drop' && event.payload.paths && event.payload.paths.length) {
        if (fileDropCallback) fileDropCallback(event.payload.paths[0]);
      }
    }).catch(() => {});
  }

  window.pantoSRT = {
    platform: navigator.platform.toLowerCase().includes('mac') ? 'darwin' : navigator.platform,
    selectMedia: () => invoke('select_media'),
    inspectMedia: (filePath) => invoke('inspect_media', { filePath }),
    transcribeMedia: (filePath, options) => invoke('transcribe_media', { filePath, options }),
    cancelTranscription: () => invoke('cancel_transcription'),
    groupClips: (words, options) => invoke('group_clips', { words, options }),
    exportSubtitles: (sourcePath, format, clips, options) => invoke('export_subtitles', { sourcePath, format, clips, options }),
    showInFolder: (filePath) => invoke('show_in_folder', { filePath }),
    setLanguage: (language) => invoke('set_language', { language }),
    setContentHeight: (height) => { invoke('set_content_height', { height }).catch(() => {}); },
    listModels: () => invoke('list_models'),
    setActiveModel: (modelId) => invoke('set_active_model', { modelId }),
    deleteModel: (modelId) => invoke('delete_model', { modelId }),
    downloadModel: (modelId) => invoke('download_model', { modelId }),
    cancelDownload: () => invoke('cancel_download'),
    onModelDownloadProgress: (callback) => {
      let unlisten = () => {};
      listen('model-download-progress', (event) => callback(event.payload)).then((fn) => { unlisten = fn; });
      return () => unlisten();
    },
    startWindowDrag: () => {
      if (tauri.window && tauri.window.getCurrentWindow) {
        tauri.window.getCurrentWindow().startDragging().catch(() => {});
      }
    },
    onFileDrop: (callback) => { fileDropCallback = callback; },
    onTranscriptionStatus: (callback) => {
      let unlisten = () => {};
      listen('transcription-status', (event) => callback(event.payload)).then((fn) => { unlisten = fn; });
      return () => unlisten();
    },
    onTranscriptionProgress: (callback) => {
      let unlisten = () => {};
      listen('transcription-progress', (event) => callback(event.payload)).then((fn) => { unlisten = fn; });
      return () => unlisten();
    }
  };
})();
