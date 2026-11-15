'use strict';
(() => {
  const sheet = $('update-sheet');
  let info = null, checking = false, installing = false;
  const close = () => { if (!installing) sheet.hidden = true; };
  $('update-later').onclick = close;
  $('update-shade').onclick = close;
  async function check(manual = false) {
    if (checking || installing) return;
    if (platformOS === 'Android') {
      if (manual) await invoke('open_ad_url', {url:'https://github.com/sams843586-oss/flyx/releases/latest'});
      return;
    }
    checking = true;
    $('check-updates').disabled = true;
    try {
      info = TAURI ? await invoke('check_update') : window.__FLYX_PREVIEW_UPDATE__ || null;
      if (info || manual) {
        $('update-title').textContent = t(info ? 'updateAvailable' : 'upToDate');
        $('update-version').textContent = 'Flyx v' + (info?.version || '0.1.0');
        $('update-description').textContent = t(info ? 'updateHint' : 'latestHint');
        $('update-notes').textContent = info?.notes || '';
        $('update-progress').hidden = true;
        $('update-install').hidden = !info;
        $('update-install').textContent = t('updateNow');
        sheet.hidden = false;
      }
    } catch (error) {
      if (manual) {
        $('update-title').textContent = t('updateError');
        $('update-description').textContent = t('updateRetry');
        $('update-notes').textContent = String(error);
        $('update-install').hidden = true;
        sheet.hidden = false;
      }
    } finally { checking = false; $('check-updates').disabled = false; }
  }
  $('check-updates').onclick = () => check(true);
  $('update-install').onclick = async () => {
    if (installing || !info) return;
    installing = true;
    $('update-install').disabled = $('update-later').disabled = true;
    $('update-description').textContent = t('downloading');
    $('update-progress').hidden = false;
    $('update-progress').removeAttribute('value');
    try {
      if (!TAURI) throw Error('Installer is only available in the desktop app');
      await invoke('install_update');
    } catch (error) {
      info = null;
      $('update-title').textContent = t('updateError');
      $('update-description').textContent = t('updateRetry');
      $('update-notes').textContent = String(error);
      $('update-install').hidden = true;
      $('update-progress').hidden = true;
    } finally {
      installing = false;
      $('update-install').disabled = $('update-later').disabled = false;
    }
  };
  if (TAURI) {
    TAURI.event.listen('update-progress', ({payload}) => {
      if (payload.total) $('update-progress').value = Math.min(100, payload.downloaded / payload.total * 100);
    });
    TAURI.event.listen('update-installing', () => { $('update-description').textContent = t('installing'); });
    setTimeout(() => check(false), 5000);
  }
  window.FlyxUpdater = {check};
})();
