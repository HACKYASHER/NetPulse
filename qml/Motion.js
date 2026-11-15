.pragma library

function enabled(window) {
    return !window || !window.prefs || window.prefs.motion !== false
}
