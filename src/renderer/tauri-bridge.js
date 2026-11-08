'use strict';

// Tauri bridge for Panto Studio. Exposes the same API shapes the three
// original apps used, backed by the unified Rust commands and events.
(function () {
  const tauri = window.__TAURI__;
  const invoke = tauri.core.invoke;
  const listen = tauri.event.listen;

  function subscribe(event, callback) {
    let unlisten = null;
    listen(event, (payload) => callback(payload.payload))
      .then((stop) => { unlisten = stop; })
      .catch(() => {});
    return () => { if (unlisten) unlisten(); };
  }

  let fileDropCallback = null;
  if (tauri.window && tauri.window.getCurrentWindow) {
    tauri.window.getCurrentWindow().onDragDropEvent((event) => {
      const payload = event.payload || {};
      if (payload.type === 'drop' && payload.paths && payload.paths.length) {
        if (window.PantoStudio && window.PantoStudio.handleDrop) {
          window.PantoStudio.handleDrop(payload.paths);
        } else if (fileDropCallback) {
          fileDropCallback(payload.paths[0]);
        }
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
    exportSubtitles: (sourcePath, format, clips, options) =>
      invoke('export_subtitles', { sourcePath, format, clips, options }),
    showInFolder: (filePath) => invoke('show_in_folder', { filePath }),
    setLanguage: (language) => invoke('set_language', { language }),
    listModels: () => invoke('list_models'),
    setActiveModel: (modelId) => invoke('set_active_model', { modelId }),
    deleteModel: (modelId) => invoke('delete_model', { modelId }),
    downloadModel: (modelId) => invoke('download_model', { modelId }),
    cancelDownload: () => invoke('cancel_download'),
    startWindowDrag: () => tauri.window.getCurrentWindow().startDragging(),
    onFileDrop: (callback) => { fileDropCallback = callback; },
    onModelDownloadProgress: (callback) => subscribe('model-download-progress', callback),
    onTranscriptionStatus: (callback) => subscribe('transcription-status', callback),
    onTranscriptionProgress: (callback) => subscribe('transcription-progress', callback)
  };

  window.pantoConvert = {
    selectFiles: (converter) => invoke('select_convert_files', { converter }),
    inspectFiles: (paths) => invoke('inspect_convert_files', { paths }),
    selectSubtitle: () => invoke('select_convert_subtitle'),
    inspectSubtitle: (path) => invoke('inspect_convert_subtitle', { path }),
    chooseFolder: () => invoke('choose_convert_folder'),
    convertMedia: (request) => invoke('convert_media', { request }),
    cancel: () => invoke('cancel_conversion'),
    showInFolder: (filePath) => invoke('show_in_folder', { filePath }),
    onProgress: (callback) => subscribe('convert-progress', callback),
    onStatus: (callback) => subscribe('convert-status', callback)
  };

  window.pantoUpscale = {
    selectVideo: () => invoke('select_upscale_video'),
    inspectVideo: (path) => invoke('inspect_upscale_video', { path }),
    chooseFolder: () => invoke('choose_upscale_folder'),
    computePlan: (args) => invoke('compute_upscale_plan', args),
    start: (request) => invoke('start_upscale', { request }),
    cancel: () => invoke('cancel_upscale'),
    status: () => invoke('ffmpeg_status'),
    showInFolder: (filePath) => invoke('show_in_folder', { filePath }),
    listModels: () => invoke('list_upscale_models'),
    setEngine: (engine) => invoke('set_upscale_engine', { engine }),
    deleteModel: (modelId) => invoke('delete_upscale_model', { modelId }),
    downloadModel: (modelId) => invoke('download_upscale_model', { modelId }),
    cancelModelDownload: () => invoke('cancel_upscale_model_download'),
    onProgress: (callback) => subscribe('upscale-progress', callback),
    onModelDownloadProgress: (callback) => subscribe('upscale-model-download-progress', callback)
  };
})();
