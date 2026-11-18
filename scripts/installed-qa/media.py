"""Physical Windows UI checks against an unmodified, installed ELIX Browser."""

import sys
import time
from itertools import groupby
from statistics import median

if sys.platform == "win32":
    import win32api
    import win32con
    import win32gui
    import win32process
    from PIL import ImageGrab
    from pywinauto import keyboard, mouse


def client_rect(hwnd):
    _, _, width, height = win32gui.GetClientRect(hwnd)
    x, y = win32gui.ClientToScreen(hwnd, (0, 0))
    return x, y, x + width, y + height


def dimensions(rect):
    return rect[2] - rect[0], rect[3] - rect[1]


def close_rect(first, second, tolerance=4):
    return all(abs(a - b) <= tolerance for a, b in zip(first, second))


def monitor(hwnd):
    return tuple(win32api.GetMonitorInfo(win32api.MonitorFromWindow(hwnd, 2))["Monitor"])


def event(ui, route, predicate, description, since=0):
    def latest():
        samples = [e for e in ui.fixture.events if e.get("kind") == "installed-media"
                   and e.get("route") == route and e["time"] >= since]
        if samples and predicate(samples[-1]):
            return samples[-1]
        return None
    return ui.wait(latest, description, timeout=18)


def original(event, token):
    return (event["token"] == token and event["sameStream"] and event["playing"]
            and event["liveAudio"] and event["mediaCount"] == 1 and event["audibleElements"] == 1)


def fills(e, hwnd):
    x, y, width, height = e["r"]
    vw, vh = e["viewport"]
    native_width, native_height = dimensions(client_rect(hwnd))
    return (abs(x) < 2 and abs(y) < 2 and abs(width-vw) < 2 and abs(height-vh) < 2
            and abs(vw * e["scale"] - native_width) < 5 and abs(vh * e["scale"] - native_height) < 5)


def start(ui, route):
    ui.navigate(ui.origin + route)
    ui.click("Start testavspilling", "Button")
    return event(ui, route, lambda e: e["playing"] and e["sameStream"] and e["t"] > 0,
                 "Original test video did not begin playing")


def fullscreen(ui):
    hwnd = ui.window.handle
    # Begin with a visibly windowed browser. Restore and resize are ordinary
    # desktop window-manager actions on the installed application's frame.
    win32gui.ShowWindow(hwnd, win32con.SW_RESTORE)
    left, top, right, bottom = monitor(hwnd)
    win32gui.SetWindowPos(hwnd, 0, left + 20, top + 16, min(970, right-left-45),
                          min(660, bottom-top-50), win32con.SWP_NOZORDER)
    sample = start(ui, "/qa-fullscreen")
    token = sample["token"]
    normal = client_rect(hwnd)
    screen = monitor(hwnd)
    assert not close_rect(normal, screen), "Expected a windowed baseline"

    def original_video(e):
        return (e["token"] == token and e["sameStream"] and e["playing"] and e["liveVideo"]
                and e["muted"] and e["mediaCount"] == 1 and e["frames"] > 0)

    def enter_and_escape(prefix):
        before = event(ui, "/qa-fullscreen", original_video, "Original muted video did not present frames")
        ui.click("Vis testvideo i fullskjerm", "Button")
        full = event(ui, "/qa-fullscreen", lambda e: original_video(e) and e["fullscreen"]
                     and e["frames"] > before["frames"] and e["t"] > before["t"]
                     and fills(e, hwnd) and close_rect(client_rect(hwnd), screen),
                     "Video did not fill both the monitor and the native ELIX client")
        ui.screenshot(prefix)
        start_time = time.monotonic()
        ui.keys("{ESC}")
        restored = event(ui, "/qa-fullscreen", lambda e: original_video(e) and not e["fullscreen"]
                         and e["frames"] > full["frames"] and e["t"] > full["t"],
                         "Escape did not restore the playing source video", start_time)
        return {"viewport_css":full["viewport"],"frames":[before["frames"],full["frames"],restored["frames"]],
                "seconds":[before["t"],full["t"],restored["t"]]}

    size = enter_and_escape("ELIX-26-video-fullscreen")
    ui.wait(lambda: close_rect(client_rect(hwnd), normal), "Windowed geometry was not restored")
    ui.keys("{F11}")
    ui.wait(lambda: close_rect(client_rect(hwnd), screen), "F11 did not enter browser fullscreen")
    f11_size = enter_and_escape("ELIX-26-video-from-F11")
    ui.wait(lambda: close_rect(client_rect(hwnd), screen), "Exiting HTML fullscreen lost prior F11 mode")
    ui.keys("{F11}")
    ui.wait(lambda: close_rect(client_rect(hwnd), normal), "Leaving F11 did not restore original window")
    ui.screenshot("ELIX-26-returned")
    return {"windowed_client":normal, "monitor":screen, "html_fullscreen":size,
            "from_F11":f11_size, "surviving_document":token}


def pdf_marker(hwnd):
    # Read screen pixels from the actual native PDF viewer rather than its UIA
    # tree, whose labels and page field vary between WebView2 versions.
    left, top, right, bottom = client_rect(hwnd)
    width, height = right-left, bottom-top
    y = top + int(height * .62)
    crop = ImageGrab.grab(bbox=(left, y, right, y+1)).convert("RGB")
    pixels = list(crop.getdata())
    yellow = [x for x, (r, g, b) in enumerate(pixels) if r>215 and g>215 and b<45]
    if not yellow:
        return None
    # A continuous yellow stripe is printed in the center of every PDF page.
    bands = (list(run) for _, run in groupby(enumerate(yellow), key=lambda pair: pair[1]-pair[0]))
    longest = max(bands, key=len)
    x0, x1 = longest[0][1], longest[-1][1]
    if x1-x0 < 10:
        return None
    sample = min(width-1, x1+12)
    r,g,b = pixels[sample]
    if r>200 and g<60 and b<60:
        page = 1
    elif b>200 and g<60 and r<60:
        page = 24
    elif g>200 and b<60 and r<60:
        page = "middle"
    else:
        return None
    return {"page":page, "stripe_px":x1-x0+1, "wheel_at":(left+(x0+x1)//2, y)}


def pdf_scroll(ui):
    ui.navigate(ui.origin + "/qa-scroll.pdf")
    hwnd = ui.window.handle

    def first():
        mark = pdf_marker(hwnd)
        return mark if mark and mark["page"] == 1 else None

    baseline = ui.wait(first, "Native PDF did not display the first of 24 pages", timeout=30)
    ui.screenshot("ELIX-27-page-1")
    coords = baseline["wheel_at"]
    mouse.click(coords=coords)
    keyboard.send_keys("{VK_CONTROL down}")
    try:
        mouse.scroll(coords=coords, wheel_dist=3)
    finally:
        keyboard.send_keys("{VK_CONTROL up}")

    def zoomed():
        mark = first()
        return mark if mark and mark["stripe_px"] > baseline["stripe_px"]*1.08 else None

    large = ui.wait(zoomed, "Ctrl+wheel did not measurably enlarge the PDF page", timeout=10)
    results = []
    for trip in range(1, 4):
        for target, direction in [(24, -1), (1, 1)]:
            deadline = time.monotonic() + 25
            gestures = 0
            while time.monotonic() < deadline:
                mouse.scroll(coords=large["wheel_at"], wheel_dist=direction*26)
                gestures += 1
                time.sleep(.17)
                mark = pdf_marker(hwnd)
                if mark and mark["page"] == target:
                    # A distinct marker at each endpoint prevents a scrollbar
                    # movement or an intermediate page from counting as success.
                    results.append({"trip":trip,"page":target,"wheel_events":gestures,"stripe_px":mark["stripe_px"]})
                    ui.screenshot(f"ELIX-27-trip-{trip}-page-{target}")
                    break
            else:
                raise AssertionError(f"Native wheel did not reach PDF page {target} on trip {trip}; last marker {pdf_marker(hwnd)}")
    return {"page_count":24, "initial_stripe_px":baseline["stripe_px"], "zoomed_stripe_px":large["stripe_px"],
            "wheel_round_trips":results}


def player_hwnd(main):
    pid = win32process.GetWindowThreadProcessId(main)[1]
    found = []
    def inspect(candidate, _):
        if (win32gui.IsWindowVisible(candidate) and win32process.GetWindowThreadProcessId(candidate)[1] == pid
                and win32gui.GetWindowText(candidate).startswith("ELIX Minispiller")):
            found.append(candidate)
        return True
    win32gui.EnumWindows(inspect, None)
    return found[0] if len(found) == 1 else None


def phase_samples(events, token, since, mode):
    # The HTTP fixture is fresh for each card. Inspect all its reports from all
    # documents: an extra document or media element is a failure even if the
    # last few successful observations came from the original player.
    all_rows = [e for e in events if e.get("kind") == "installed-media" and e.get("route") == "/qa-miniplayer"]
    assert all(e["token"] == token and e["mediaCount"] == 1 for e in all_rows), "Extra document or media element during playback"
    rows = [e for e in all_rows if e["time"] >= since][-3:]
    if len(rows) < 3 or any(not e["sameStream"] or not e.get("pcm") or not e["liveAudio"] for e in rows):
        return None
    if any(b["pcm"]["clock"] <= a["pcm"]["clock"] for a, b in zip(rows,rows[1:])):
        return None
    if mode == "tone":
        if not all(original(e,token) and .075 <= e["pcm"]["rms"] <= .102
                   and e["pcm"]["toneFraction"] >= .93 for e in rows):
            return None
    elif mode in ("muted", "paused"):
        if not all(e["audibleElements"] == 0 and e["muted"] == (mode == "muted")
                   and e["playing"] == (mode == "muted") and e["pcm"]["rms"] < .0001 for e in rows):
            return None
    else:
        raise ValueError(f"Unknown playback observation {mode}")
    if mode == "paused":
        if abs(rows[-1]["t"] - rows[0]["t"]) > .02:
            return None
    elif rows[-1]["t"] <= rows[0]["t"] + .05 or rows[-1]["frames"] <= rows[0]["frames"]:
        return None
    return {"rms":[round(e["pcm"]["rms"],6) for e in rows],
            "fraction_at_440_hz":[round(e["pcm"]["toneFraction"],4) for e in rows],
            "media_seconds":[rows[0]["t"],rows[-1]["t"]],
            "presented_frames":[rows[0]["frames"],rows[-1]["frames"]],
            "sample_rate":rows[-1]["pcm"]["sampleRate"],"samples_per_block":rows[-1]["pcm"]["samples"]}


def digital_output(ui, token, popup=None):
    # Physical keys operate the site's original media element. Check both
    # directions of mute and pause so recorded/replayed telemetry cannot pass.
    if popup:
        left, top, right, bottom = client_rect(popup)
        stamp = time.monotonic()
        mouse.click(button="right",coords=(left+(right-left)//3,top+(bottom-top)//3))
        event(ui,"/qa-miniplayer",lambda e: e["token"]==token and e["focusClicks"] and e["focused"],
              "Click did not focus the original popup WebView",stamp)
    else:
        ui.click("Start testavspilling", "Button")
    phases = {}
    for step, key, mode in [("playing",None,"tone"),("mute","q","muted"),("unmute","q","tone"),
                            ("pause","w","paused"),("resume","w","tone")]:
        if key:
            keyboard.send_keys(key, vk_packet=False)
        since = time.monotonic()
        phases[step] = ui.wait(lambda: phase_samples(ui.fixture.events,token,since,mode),
                               f"Post-element PCM did not verify {step}",timeout=10)
    return phases


def output_meter(ui, phase):
    import comtypes
    import psutil
    from pycaw.constants import CLSID_MMDeviceEnumerator, EDataFlow, DEVICE_STATE
    from pycaw.pycaw import IMMDeviceEnumerator, IAudioSessionManager2, IAudioSessionControl2, IAudioMeterInformation
    enum = comtypes.CoCreateInstance(CLSID_MMDeviceEnumerator, IMMDeviceEnumerator, comtypes.CLSCTX_INPROC_SERVER)
    devices = enum.EnumAudioEndpoints(EDataFlow.eRender.value,DEVICE_STATE.ACTIVE.value)
    count = devices.GetCount()
    if count == 0:
        # A positively enumerated empty device collection is different from a
        # silent device, API failure, or inability to inspect an active session.
        return {"status":"unavailable","active_render_endpoints":0,"reason":"No active Windows render endpoint"}
    managers = []
    for index in range(count):
        device = devices.Item(index)
        interface = device.Activate(IAudioSessionManager2._iid_,comtypes.CLSCTX_ALL,None)
        managers.append(interface.QueryInterface(IAudioSessionManager2))
    pid = win32process.GetWindowThreadProcessId(ui.window.handle)[1]
    deadline = time.monotonic() + 8
    readings = []
    while time.monotonic() < deadline:
        tree = {p.pid for p in psutil.Process(pid).children(recursive=True)} | {pid}
        active = []
        for endpoint, manager in enumerate(managers):
            sessions = manager.GetSessionEnumerator()
            for index in range(sessions.GetCount()):
                session = sessions.GetSession(index).QueryInterface(IAudioSessionControl2)
                owner = session.GetProcessId()
                if owner not in tree or session.GetState() != 1:
                    continue
                peak = session.QueryInterface(IAudioMeterInformation).GetPeakValue()
                if peak > .0005:
                    active.append(((endpoint,session.GetSessionInstanceIdentifier()),owner,float(peak)))
        if len(active)>1:
            raise AssertionError(f"{phase}: {len(active)} ELIX render sessions had simultaneous output")
        if active:
            readings.append(active[0])
            if len(readings) >= 4 and len({r[0] for r in readings[-4:]}) == 1:
                rows=readings[-4:]
                return {"status":"measured","active_render_endpoints":count,"render_sessions_with_output":1,
                        "process":rows[0][1],"median_peak":round(median(r[2] for r in rows),5),"samples":len(rows)}
        else:
            readings.clear()
        time.sleep(.14)
    raise RuntimeError(f"{phase}: active Windows render endpoint(s) but no measurable ELIX signal")


def miniplayer(ui):
    route = "/qa-miniplayer"
    initial = start(ui, route)
    token = initial["token"]
    event(ui, route, lambda e: original(e,token) and e["siteChrome"]=="visible", "Missing original page or sound source")
    meters = {}
    audio_failures = {}
    def sample_output(phase):
        try:
            meters[phase] = output_meter(ui, phase)
        except Exception as error:
            # Preserve independent geometry and continuity evidence if the
            # OS has an active but silent device or the native API fails.
            audio_failures[phase] = str(error) or type(error).__name__
    pcm = {"in_tab":digital_output(ui,token)}
    sample_output("in_tab")
    ui.click("Åpne video eller musikk i minispiller", "Button")
    hwnd = ui.wait(lambda: player_hwnd(ui.window.handle), "Installed browser did not show its miniplayer")
    assert win32gui.GetWindowLong(hwnd,win32con.GWL_EXSTYLE)&win32con.WS_EX_TOPMOST, "Miniplayer was not always on top"
    popup = event(ui,route,lambda e: original(e,token) and e["siteChrome"]=="hidden" and fills(e,hwnd),
                  "Popup did not isolate and fill the live video")
    pcm["popup"] = digital_output(ui,token,hwnd)
    sample_output("popup")
    ui.screenshot("ELIX-23-isolated-player")
    left,top,right,bottom=win32gui.GetWindowRect(hwnd)
    anchor = None
    for offset in range(2, 13):
        xy = (right-offset, bottom-offset)
        packed = (xy[0]&0xffff) | ((xy[1]&0xffff)<<16)
        if win32gui.SendMessage(hwnd, win32con.WM_NCHITTEST, 0, packed) == win32con.HTBOTTOMRIGHT:
            anchor = xy
            break
    assert anchor, "Native miniplayer did not expose a draggable resize corner"
    mouse.press(coords=anchor)
    try:
        mouse.move(coords=(anchor[0]-125,anchor[1]-75))
    finally:
        mouse.release(coords=(anchor[0]-125,anchor[1]-75))
    resized=event(ui,route,lambda e: original(e,token) and e["siteChrome"]=="hidden" and fills(e,hwnd)
                  and e["viewport"][0] < popup["viewport"][0]-45 and e["t"]>popup["t"],
                  "Dragging the miniplayer corner did not resize the playing video")
    ui.screenshot("ELIX-23-resized-player")
    # The popup is still the active window after the physical resize.
    assert win32gui.GetForegroundWindow()==hwnd, "The resized player did not own keyboard focus"
    keyboard.send_keys("%{F4}")
    ui.wait(lambda: not player_hwnd(ui.window.handle),"Closing miniplayer did not return its original tab")
    restored = event(ui,route,lambda e: original(e,token) and e["siteChrome"]=="visible" and e["t"]>resized["t"],
                     "Returning the player lost its original document, stream, or site menu")
    pcm["returned"] = digital_output(ui,token)
    sample_output("returned")
    tokens = {e["token"] for e in ui.fixture.events if e.get("kind")=="installed-media" and e.get("route")==route}
    assert tokens=={token}, f"An extra media document was instantiated: {len(tokens)} distinct documents"
    ui.screenshot("ELIX-23-returned-player")
    result = {"single_document":token,"media_elements":restored["mediaCount"],"audible_elements":restored["audibleElements"],
            "initial_popup_css":popup["viewport"],"resized_popup_css":resized["viewport"],
            "playback_seconds":[initial["t"],popup["t"],resized["t"],restored["t"]],"decoded_element_pcm":pcm,"windows_audio":meters}
    if audio_failures:
        raise AssertionError(f"Output verification incomplete: {audio_failures}. Measured player evidence: {result}")
    return result


CASES={"ELIX-26":fullscreen,"ELIX-27":pdf_scroll,"ELIX-23":miniplayer}
