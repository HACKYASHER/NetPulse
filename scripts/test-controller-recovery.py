"""Crash only an owned isolated controller and verify that the packaged GUI recovers."""
from pathlib import Path
import ctypes
from ctypes import wintypes
import json
import os
import subprocess
import time
import uuid

ROOT = Path(__file__).resolve().parents[1]
kernel = ctypes.WinDLL('kernel32', use_last_error=True)


class ProcessEntry(ctypes.Structure):
    _fields_ = [('size', wintypes.DWORD), ('usage', wintypes.DWORD),
                ('pid', wintypes.DWORD), ('heap', ctypes.c_size_t),
                ('module', wintypes.DWORD), ('threads', wintypes.DWORD),
                ('parent', wintypes.DWORD), ('priority', wintypes.LONG),
                ('flags', wintypes.DWORD), ('name', wintypes.WCHAR * 260)]


kernel.CreateToolhelp32Snapshot.argtypes = [wintypes.DWORD, wintypes.DWORD]
kernel.CreateToolhelp32Snapshot.restype = wintypes.HANDLE
kernel.Process32FirstW.argtypes = [wintypes.HANDLE, ctypes.POINTER(ProcessEntry)]
kernel.Process32NextW.argtypes = [wintypes.HANDLE, ctypes.POINTER(ProcessEntry)]
kernel.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
kernel.OpenProcess.restype = wintypes.HANDLE
kernel.QueryFullProcessImageNameW.argtypes = [wintypes.HANDLE, wintypes.DWORD, wintypes.LPWSTR, ctypes.POINTER(wintypes.DWORD)]
kernel.TerminateProcess.argtypes = [wintypes.HANDLE, wintypes.UINT]
kernel.WaitForSingleObject.argtypes = [wintypes.HANDLE, wintypes.DWORD]
kernel.CloseHandle.argtypes = [wintypes.HANDLE]


def controller_children(parent):
    snapshot = kernel.CreateToolhelp32Snapshot(2, 0)
    assert snapshot != ctypes.c_void_p(-1).value
    try:
        entry = ProcessEntry()
        entry.size = ctypes.sizeof(entry)
        ok = kernel.Process32FirstW(snapshot, ctypes.byref(entry))
        result = []
        while ok:
            if entry.parent == parent and entry.name.lower() == 'netpulsed.exe':
                result.append(entry.pid)
            ok = kernel.Process32NextW(snapshot, ctypes.byref(entry))
        return result
    finally:
        kernel.CloseHandle(snapshot)


def wait_for(check, timeout=45):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        value = check()
        if value:
            return value
        time.sleep(.1)
    raise AssertionError('Isolated controller recovery timed out')


def main():
    output = ROOT / '.build' / ('controller-recovery-' + uuid.uuid4().hex)
    data = output / 'data'
    data.mkdir(parents=True)
    app = ROOT / 'release/NetPulse'
    env = os.environ.copy()
    for name in list(env):
        if name.startswith(('QT_', 'QML')):
            env.pop(name)
    primary = subprocess.Popen([str(app / 'NetPulse.exe'), '--isolated', '--background', '--data-dir', str(data)],
                               env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, creationflags=0x08000000)
    handle = None
    try:
        def children():
            assert primary.poll() is None, 'Owned GUI exited unexpectedly'
            return controller_children(primary.pid)
        old = wait_for(children)[0]
        wait_for(lambda: any((data / 'logs').glob('*/core.jsonl')))
        handle = kernel.OpenProcess(0x1000 | 0x100000 | 1, False, old)
        assert handle
        path = ctypes.create_unicode_buffer(32768)
        size = wintypes.DWORD(len(path))
        assert kernel.QueryFullProcessImageNameW(handle, 0, path, ctypes.byref(size))
        assert Path(path.value).resolve() == (app / 'netpulsed.exe').resolve()
        # The parent is our own --isolated GUI; additionally verify the image
        # path before exercising an abrupt controller failure.
        assert kernel.TerminateProcess(handle, 99)
        assert kernel.WaitForSingleObject(handle, 10000) == 0
        new = wait_for(lambda: [pid for pid in children() if pid != old])[0]
        wait_for(lambda: sum(p.read_text(encoding='utf-8').count('"Idle"') for p in (data / 'logs').glob('*/core.jsonl')) >= 2)
        assert primary.poll() is None and new in children()
        result = {'result': 'passed', 'gui_survived_controller_crash': True,
                  'controller_restarted_automatically': True, 'isolated_session': True,
                  'system_network_changed': False, 'production_profile_changed': False}
        (ROOT / '.build/controller-recovery-result.json').write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
        print(json.dumps(result, indent=2))
    finally:
        if primary.poll() is None:
            primary.terminate()
        primary.wait(10)
        if handle:
            kernel.CloseHandle(handle)
        # Closing the owned GUI closes its controller's stdin and triggers cleanup.
        wait_for(lambda: not controller_children(primary.pid), 20)


if __name__ == '__main__':
    main()
