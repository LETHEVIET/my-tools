'use strict';
(() => {
  const $ = (id) => document.getElementById(id);
  const nativeCameraInput = $('nativeCameraInput');
  const libraryInput = $('libraryInput');
  const shutter = $('snapButton');
  const libraryButton = $('libraryButton');
  const flipButton = $('flipButton');
  const torchButton = $('torchButton');
  const openCameraButton = $('openCameraButton');
  const video = $('liveVideo');
  const preview = $('photoPreview');
  const placeholder = $('placeholder');
  const overlay = $('uploadOverlay');
  const retryButton = $('retryButton');
  const cameraSupported = !!(window.isSecureContext && navigator.mediaDevices?.getUserMedia);

  let token = '';
  let stream = null;
  let facing = 'environment';
  let cameraWanted = cameraSupported;
  let cameraGeneration = 0;
  let torchEnabled = false;
  let previewUrl = null;
  let lastFile = null;
  let busy = false;

  function pairingToken() {
    const match = location.hash.match(/^#pair=([\w-]+)$/);
    if (match) {
      localStorage.setItem('snap_to_prompt_pairing', match[1]);
      history.replaceState(null, '', location.pathname);
    }
    return localStorage.getItem('snap_to_prompt_pairing') || '';
  }

  function report(kind, title, detail) {
    $('result').classList.toggle('error', kind === 'error');
    $('resultIcon').textContent = kind === 'error' ? '!' : kind === 'ok' ? '✓' : '↗';
    $('resultTitle').textContent = title;
    $('resultDetail').textContent = detail;
  }

  function connection(online, message) {
    $('connection').classList.toggle('online', online);
    $('connection').classList.toggle('offline', !online);
    $('connectionText').textContent = message;
  }

  async function status() {
    if (!token) {
      connection(false, 'Not paired');
      report('error', 'Pair your iPhone first', 'Scan the pairing QR code shown on Windows.');
      return;
    }
    try {
      const response = await fetch('/api/status', {
        headers: {'Authorization': 'Bearer ' + token},
        cache: 'no-store'
      });
      if (response.status === 401) throw new Error('Pairing expired. Scan the new PC QR code.');
      if (!response.ok) throw new Error('PC receiver unavailable.');
      connection(true, 'PC connected');
    } catch (error) {
      connection(false, 'Disconnected');
      if (!busy && !lastFile) report('error', 'Cannot reach your PC', error.message || 'Check Wi-Fi.');
    }
  }

  function setBusy(value) {
    busy = value;
    shutter.disabled = value;
    libraryButton.disabled = value;
    flipButton.disabled = value;
    openCameraButton.disabled = value;
    torchButton.disabled = value;
    overlay.hidden = !value;
  }

  function showPhoto(file) {
    if (previewUrl) URL.revokeObjectURL(previewUrl);
    previewUrl = URL.createObjectURL(file);
    preview.src = previewUrl;
    preview.hidden = false;
    video.hidden = true;
    placeholder.hidden = true;
    $('cameraMode').textContent = 'Photo preview';
  }

  function showLive() {
    if (!stream) return;
    video.hidden = false;
    preview.hidden = true;
    placeholder.hidden = true;
    $('cameraMode').textContent = 'Live camera';
    $('stageHint').textContent = 'Tap the shutter to paste into your PC';
  }

  function stopCamera() {
    cameraGeneration++;
    if (stream) stream.getTracks().forEach(track => track.stop());
    stream = null;
    torchEnabled = false;
    video.pause();
    video.srcObject = null;
    video.hidden = true;
    flipButton.hidden = true;
    torchButton.hidden = true;
    torchButton.textContent = '⚡ Flash';
  }

  function showCameraFallback() {
    $('cameraMode').textContent = cameraSupported ? 'Camera unavailable' : 'iPhone camera';
    $('captureCaption').textContent = 'The shutter opens the native iPhone camera';
    $('stageHint').textContent = 'Photo Library is always available';
    openCameraButton.hidden = !cameraSupported;
    flipButton.hidden = true;
    torchButton.hidden = true;
    if (preview.hidden) placeholder.hidden = false;
  }

  async function startCamera() {
    if (!cameraSupported || busy) return;
    stopCamera();
    cameraWanted = true;
    const generation = cameraGeneration;
    $('cameraMode').textContent = 'Opening camera…';
    $('placeholderTitle').textContent = 'Opening camera';
    $('placeholderDetail').textContent = 'Allow camera access when iOS asks.';
    placeholder.hidden = false;
    preview.hidden = true;
    openCameraButton.hidden = true;
    try {
      const opened = await navigator.mediaDevices.getUserMedia({
        audio: false,
        video: {facingMode: {ideal: facing}, width: {ideal: 1920}, height: {ideal: 1080}}
      });
      if (generation !== cameraGeneration || document.hidden) {
        opened.getTracks().forEach(track => track.stop());
        return;
      }
      stream = opened;
      video.srcObject = opened;
      video.classList.toggle('mirrored', facing === 'user');
      await video.play();
      if (generation !== cameraGeneration) return;
      showLive();
      flipButton.hidden = false;
      openCameraButton.hidden = true;
      $('captureCaption').textContent = 'Snap & paste without leaving the app';
      const track = stream.getVideoTracks()[0];
      const capabilities = track?.getCapabilities?.();
      torchButton.hidden = !capabilities?.torch;
    } catch (error) {
      if (generation !== cameraGeneration) return;
      stopCamera();
      cameraWanted = false; // Do not repeatedly prompt after permission denial.
      $('placeholderTitle').textContent = 'Camera access needed';
      $('placeholderDetail').textContent = 'Tap Camera to try again, or use your Photo Library.';
      showCameraFallback();
      report('error', 'Camera did not open', 'Allow camera permission for this site in Safari.');
    }
  }

  async function flipCamera() {
    if (busy || !stream) return;
    facing = facing === 'environment' ? 'user' : 'environment';
    await startCamera();
  }

  async function toggleTorch() {
    const track = stream?.getVideoTracks()[0];
    if (!track || busy) return;
    try {
      await track.applyConstraints({advanced: [{torch: !torchEnabled}]});
      torchEnabled = !torchEnabled;
      torchButton.textContent = torchEnabled ? '⚡ Flash on' : '⚡ Flash';
    } catch {
      torchButton.hidden = true; // iOS Safari may report torch but not permit it.
    }
  }

  async function imageToJpeg(file) {
    // Convert Photo Library images, including HEIC where Safari can decode it,
    // to a format the small Rust receiver supports, with size and orientation normalized.
    let source = null;
    let objectUrl = null;
    try {
      try {
        source = await createImageBitmap(file, {imageOrientation: 'from-image'});
      } catch {
        objectUrl = URL.createObjectURL(file);
        const picture = new Image();
        picture.src = objectUrl;
        await picture.decode();
        source = picture;
      }
      const width = source.width || source.naturalWidth;
      const height = source.height || source.naturalHeight;
      if (!width || !height) throw new Error('Could not read photo dimensions');
      const scale = Math.min(1, 2400 / Math.max(width, height));
      const canvas = document.createElement('canvas');
      canvas.width = Math.max(1, Math.round(width * scale));
      canvas.height = Math.max(1, Math.round(height * scale));
      const context = canvas.getContext('2d');
      if (!context) throw new Error('Canvas unavailable');
      context.fillStyle = '#ffffff';
      context.fillRect(0, 0, canvas.width, canvas.height);
      context.drawImage(source, 0, 0, canvas.width, canvas.height);
      const blob = await new Promise((resolve) => canvas.toBlob(resolve, 'image/jpeg', 0.88));
      if (!blob) throw new Error('Photo conversion failed');
      return new File([blob], `snap-${Date.now()}.jpg`, {type: 'image/jpeg'});
    } catch {
      // JPEG and PNG are directly supported by the receiver.
      if (['image/jpeg', 'image/png'].includes(file.type)) return file;
      throw new Error('This photo could not be decoded. Choose a JPEG or PNG image.');
    } finally {
      if (source?.close) source.close();
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    }
  }

  async function sendPhoto(file, alreadyJpeg = false, count = 1, total = 1) {
    if (busy) return false;
    if (!token) {
      report('error', 'Not paired', 'Scan the QR code shown on your Windows PC.');
      return false;
    }
    setBusy(true);
    retryButton.hidden = true;
    $('uploadLabel').textContent = total > 1 ? `Sending ${count} of ${total}…` : 'Sending to PC…';
    report('neutral', total > 1 ? `Sending image ${count} of ${total}…` : 'Sending image…', 'Keep the AI prompt focused on Windows.');
    try {
      const prepared = alreadyJpeg ? file : await imageToJpeg(file);
      lastFile = prepared;
      showPhoto(prepared);
      const response = await fetch('/api/photo', {
        method: 'POST',
        headers: {'Authorization': 'Bearer ' + token, 'Content-Type': prepared.type},
        body: prepared, cache: 'no-store'
      });
      const result = await response.json().catch(() => ({}));
      if (!response.ok) throw new Error(result.error || `Upload failed (${response.status})`);
      report('ok', total > 1 ? `Pasted ${count} of ${total} ✓` : 'Sent to Windows ✓',
        result.message || 'Image sent to the PC clipboard.');
      connection(true, 'PC connected');
      lastFile = null;
      return true;
    } catch (error) {
      report('error', 'Photo not pasted', error.message || 'Check your connection and try again.');
      retryButton.hidden = !lastFile;
      return false;
    } finally {
      setBusy(false);
      if (!lastFile && stream) showLive();
    }
  }

  async function sendLibraryFiles(files) {
    if (busy || !files.length) return;
    // Sequential uploads preserve the order of Ctrl+V inserts on Windows.
    for (let i = 0; i < files.length; i++) {
      const ok = await sendPhoto(files[i], false, i + 1, files.length);
      if (!ok) return;
    }
    // The iOS photo picker may suspend the camera while Safari is backgrounded.
    // Restore the embedded preview once every selected photo was sent.
    if (cameraWanted && cameraSupported && !stream && !document.hidden) void startCamera();
  }

  async function snapLive() {
    if (!stream || busy) return;
    const width = video.videoWidth, height = video.videoHeight;
    if (!width || !height) {
      report('error', 'Camera is still starting', 'Try again in a moment.');
      return;
    }
    const scale = Math.min(1, 2400 / Math.max(width, height));
    const canvas = document.createElement('canvas');
    canvas.width = Math.round(width * scale);
    canvas.height = Math.round(height * scale);
    canvas.getContext('2d').drawImage(video, 0, 0, canvas.width, canvas.height);
    const blob = await new Promise((resolve) => canvas.toBlob(resolve, 'image/jpeg', 0.9));
    if (!blob) return report('error', 'Capture failed', 'Try again or select from Library.');
    await sendPhoto(new File([blob], `snap-${Date.now()}.jpg`, {type: 'image/jpeg'}), true);
  }

  shutter.addEventListener('click', () => {
    if (busy) return;
    if (stream) void snapLive();
    else if (cameraSupported && cameraWanted) void startCamera();
    else nativeCameraInput.click();
  });
  libraryButton.addEventListener('click', () => { if (!busy) libraryInput.click(); });
  openCameraButton.addEventListener('click', () => { cameraWanted = true; void startCamera(); });
  flipButton.addEventListener('click', flipCamera);
  torchButton.addEventListener('click', toggleTorch);

  nativeCameraInput.addEventListener('change', () => {
    const file = nativeCameraInput.files?.[0];
    nativeCameraInput.value = '';
    if (file) void sendPhoto(file);
  });
  libraryInput.addEventListener('change', () => {
    const files = Array.from(libraryInput.files || []);
    libraryInput.value = '';
    void sendLibraryFiles(files);
  });
  retryButton.addEventListener('click', () => { if (lastFile) void sendPhoto(lastFile, true); });
  document.addEventListener('visibilitychange', () => {
    if (document.hidden) stopCamera();
    else if (cameraWanted && cameraSupported && !busy) void startCamera();
  });
  window.addEventListener('pagehide', stopCamera);

  token = pairingToken();
  if (!cameraSupported) {
    $('secureNotice').hidden = false;
    $('placeholderTitle').textContent = 'Ready to take a photo';
    $('placeholderDetail').textContent = 'Tap the shutter for the iPhone camera, or choose from Library.';
    showCameraFallback();
  } else {
    void startCamera();
  }
  void status();
  setInterval(() => { if (!document.hidden) void status(); }, 15000);
  if ('serviceWorker' in navigator && window.isSecureContext) {
    navigator.serviceWorker.register('/sw.js').catch(() => {});
  }
})();
