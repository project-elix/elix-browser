"""Installed ELIX UI checks for drag feedback, tab groups and bookmark groups."""
import ctypes
from ctypes import wintypes
import sqlite3
import time
from collections import Counter
from contextlib import closing

from PIL import ImageChops, ImageGrab
from pywinauto import mouse


def title(slug):
    return f"ELIX {slug}"


def open_pages(ui, slugs):
    for index, slug in enumerate(slugs):
        (ui.navigate if index == 0 else ui.new_tab)(ui.page(slug))
        ui.find(title(slug), "TabItem")


def relevant_tabs(ui, urls):
    return [tab for tab in ui.session()["tabs"] if tab["url"] in urls]


def tab_order(ui, urls):
    return [tab["url"] for tab in relevant_tabs(ui, urls)]


def center(control, fraction=0.5, vertical=0.5):
    r = control.rectangle()
    if r.width() < 8 or r.height() < 8:
        raise AssertionError(f"Control has no usable hit area: {control.window_text()!r} {r}")
    return (r.left + round((r.width() - 1) * fraction),
            r.top + round((r.height() - 1) * vertical))


def drag(start, end, held=None):
    """Send enough real mouse motion for WebView2 to acquire pointer capture."""
    mouse.move(coords=start)
    mouse.press(button="left", coords=start)
    try:
        for step in range(1, 13):
            point = tuple(round(a + (b - a) * step / 12) for a, b in zip(start, end))
            mouse.move(coords=point)
            time.sleep(0.025)
        if held:
            held()
    finally:
        mouse.release(button="left", coords=end)


def drag_feedback(ui):
    slugs = ["Drag-A", "Drag-B", "Drag-C"]
    open_pages(ui, slugs)
    # The internal page leaves the entire ghost visible in the shell. No external
    # content, animations or child WebView can contribute pixels in the sample.
    ui.new_tab("elix://history")
    def active_history():
        session = ui.session()
        return next((t["id"] for t in session["tabs"] if t["id"] == session["active"]
                     and t["url"] == "elix://history"), None)
    active = ui.wait(active_history, "History did not become the active tab")
    urls = [ui.page(slug) for slug in slugs]
    before = tab_order(ui, urls)
    if before != urls:
        raise AssertionError(f"Unexpected initial tab order: {before}")
    source = center(ui.find(title(slugs[2]), "TabItem"))
    target = center(ui.find(title(slugs[0]), "TabItem"), fraction=0.15)
    scale = ctypes.windll.user32.GetDpiForWindow(wintypes.HWND(ui.window.handle)) / 96 or 1
    x, y = target
    # Interior of the 244 x 54 CSS-pixel preview, below the tab strip and away
    # from the insertion line, source highlight, cursor and its shadow.
    sample = (x + round(25 * scale), y + round(23 * scale),
              x + round(172 * scale), y + round(51 * scale))
    baseline = ImageGrab.grab(bbox=sample).convert("RGB")
    ui.screenshot("ELIX-19-before")
    measurement = {}

    def changed_fraction():
        now = ImageGrab.grab(bbox=sample).convert("RGB")
        delta = ImageChops.difference(baseline, now).convert("L")
        return sum(delta.histogram()[6:]) / (delta.width * delta.height)

    def while_held():
        def visible_ghost():
            fraction = changed_fraction()
            return fraction if fraction >= 0.08 else False
        measurement["changed_pixel_fraction"] = round(
            ui.wait(visible_ghost, "No visible drag preview over the resting toolbar", timeout=5), 3)
        ui.screenshot("ELIX-19-drag-held")
        if tab_order(ui, urls) != before:
            raise AssertionError("Tab order changed before release")

    drag(source, target, while_held)
    expected = [urls[2], urls[0], urls[1]]
    ui.wait(lambda: tab_order(ui, urls) == expected, "Drop did not reorder the source tab")
    if ui.session()["active"] != active:
        raise AssertionError("Dragging a background tab changed the active tab")
    ui.screenshot("ELIX-19-after")
    return {**measurement, "before": before, "after": expected, "active_unchanged": True}


def visible_buttons(ui, predicate):
    result = []
    window = ui.window.rectangle()
    for button in ui.window.descendants(control_type="Button"):
        if predicate(button.window_text()) and button.is_visible():
            rect = button.rectangle()
            x, y = (rect.left + rect.right) // 2, (rect.top + rect.bottom) // 2
            if rect.width() and rect.height() and window.left <= x < window.right and window.top <= y < window.bottom:
                result.append(button)
    return result


def ensure_rail(ui):
    if visible_buttons(ui, lambda name: name == "Vis bare ikoner"):
        return
    if not visible_buttons(ui, lambda name: name == "Utvid sidemeny"):
        ui.click("Vis eller skjul sidemeny", "Button")
    ui.click("Utvid sidemeny", "Button")
    ui.find("Vis bare ikoner", "Button")


def tab_groups(ui):
    slugs = ["Group-A", "Group-B", "Group-C", "Outside"]
    group = "QA-tabgroup"
    urls = [ui.page(slug) for slug in slugs]
    open_pages(ui, slugs)
    ensure_rail(ui)
    ui.find(title(slugs[0]), "TabItem").click_input()
    def selected_source():
        session = ui.session()
        return any(t["url"] == urls[0] and t["id"] == session["active"] for t in session["tabs"])
    ui.wait(selected_source, "Selected source tab did not activate")
    ui.click("Innstillinger og hurtigvalg", "Button")
    ui.click("Faner", "Button")
    ui.find("Fanegruppe").click_input()
    ui.keys("^a" + group)
    ui.click("Lagre fanegruppe", "Button")
    ui.wait(lambda: any(t["url"] == urls[0] and t.get("group") == group
                        for t in relevant_tabs(ui, urls)), "Group assignment was not saved")
    ui.keys("{ESC}")
    active = ui.session()["active"]

    def members():
        return [t["url"] for t in relevant_tabs(ui, urls) if t.get("group") == group]

    for idx in (1, 2):
        # Accessibility names on group headings include the live member count.
        heading = ui.wait(lambda: next(iter(visible_buttons(ui, lambda n: n.startswith(group))), None),
                          "Group heading missing from the expanded rail")
        drag(center(ui.find(title(slugs[idx]), "TabItem")), center(heading))
        ui.wait(lambda: members() == urls[:idx + 1], f"Failed to drop {slugs[idx]} into the group")
    before = members()
    ui.screenshot("ELIX-20-three-members")
    outside = ui.find("Åpne " + title(slugs[3]), "Button")
    drag(center(ui.find("Åpne " + title(slugs[1]), "Button")), center(outside, vertical=0.17))
    ui.wait(lambda: members() == [urls[0], urls[2]], "Extracting one tab changed its siblings")
    after = relevant_tabs(ui, urls)
    if Counter(t["url"] for t in after) != Counter(urls):
        raise AssertionError("A tab disappeared or was duplicated after extracting a member")
    if any(t.get("group") for t in after if t["url"] in (urls[1], urls[3])):
        raise AssertionError("Extracted or unrelated tab still belongs to a group")
    if ui.session()["active"] != active:
        raise AssertionError("Tab group drag changed the active tab")
    ui.screenshot("ELIX-20-after-extract")
    return {"before": before, "remaining_members": members(), "tab_order": tab_order(ui, urls),
            "extracted": urls[1], "active_unchanged": True}


def bookmarks(ui):
    slug = "Bookmark-28"
    name = "QA-Bookmarks"
    url = ui.page(slug)
    open_pages(ui, [slug])
    ui.click("Legg til bokmerke", "Button")

    def membership():
        # Read only; the installed application performs every write in this case.
        uri = (ui.profile_root / "elix.sqlite").resolve().as_uri() + "?mode=ro"
        with closing(sqlite3.connect(uri, uri=True, timeout=2)) as db:
            return db.execute("""
                SELECT b.id, b.profile, COALESCE(g.name, '') FROM bookmarks AS b
                LEFT JOIN bookmark_group_entries AS g ON g.bookmark = b.id AND g.profile = b.profile
                WHERE b.url = ?
                """, (url,)).fetchall()

    rows = ui.wait(membership, "Clicking the star did not save the bookmark")
    if len(rows) != 1 or rows[0][2]:
        raise AssertionError(f"Unexpected initial bookmark state: {rows!r}")
    bookmark_id, profile, _ = rows[0]
    expected = [(bookmark_id, profile, name)]
    ui.navigate("elix://bookmarks")
    ui.click("Ny gruppe", "Button")
    ui.find("Gruppenavn", "Edit").click_input()
    ui.keys(name)
    ui.click("Lagre", "Button")
    ui.find(f"Bokmerkegruppe {name}", "Button")
    more = "Valg for " + title(slug)
    ui.click(more, "Button")
    ui.click("Flytt til gruppe …", "MenuItem")
    ui.find("Flytt til bokmerkegruppe", "ComboBox").click_input()
    ui.keys("{END}{ENTER}")
    ui.click("Lagre", "Button")
    ui.wait(lambda: membership() == expected, "Moving the bookmark did not persist its group")
    ui.screenshot("ELIX-28-before-restart")
    ui.restart()
    ui.navigate("elix://bookmarks")
    restored_item = ui.find(more, "Button")
    if membership() != expected:
        raise AssertionError("Saved bookmark identity, profile or group changed on restart")
    # The page alone has a per-bookmark options button. In the group its tab
    # order is header, bookmark, options. The toolbar has a separate group
    # header with the same accessible name and independent expansion state.
    restored_item.set_focus()
    ui.wait(restored_item.has_keyboard_focus, "Could not focus the restored page bookmark")
    ui.keys("+{TAB}+{TAB}")
    def focused_page_folder():
        headers = ui.window.descendants(title=f"Bokmerkegruppe {name}", control_type="Button")
        return next((control for control in headers if control.has_keyboard_focus()), None)
    folder = ui.wait(focused_page_folder, "The restored bookmark is not preceded by its own group header")
    ui.keys("{ENTER}")
    ui.wait(lambda: not ui.window.descendants(title=more, control_type="Button"),
            "The restored bookmark stayed outside its collapsed group")
    ui.wait(folder.has_keyboard_focus, "Folding the page group lost the keyboard focus")
    ui.keys("{ENTER}")
    ui.find(more, "Button")
    ui.screenshot("ELIX-28-restored")
    return {"url": url, "bookmark_id": bookmark_id, "profile": profile, "group": name,
            "restored": True, "collapsed_and_reopened": True}


CASES = {"ELIX-19": drag_feedback, "ELIX-20": tab_groups, "ELIX-28": bookmarks}
