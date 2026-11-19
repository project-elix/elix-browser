"""Acceptance checks against the installed shell, rendered pixels and profile database."""
import base64
import io
import sqlite3
import time
from contextlib import closing
from urllib.parse import parse_qs, urlsplit

from PIL import Image


def current_tab(ui):
    session = ui.session()
    return next((tab for tab in session.get("tabs", []) if tab["id"] == session.get("active")), {})


def address_value(ui):
    return ui.find("Adressefelt").iface_value.CurrentValue


def visit(ui, slug):
    url = ui.page(slug)
    ui.navigate(url)
    ui.find("ELIX " + slug, "TabItem", timeout=25)
    ui.wait(lambda: current_tab(ui).get("url") == url, "Navigasjonen mangler i fanens økt")
    return url


def query_db(ui, statement, parameters=()):
    path = ui.profile_root / "elix.sqlite"
    with closing(sqlite3.connect(path.resolve().as_uri() + "?mode=ro", uri=True, timeout=3)) as db:
        return db.execute(statement, parameters).fetchall()


def new_tab_focus(ui):
    from pywinauto import mouse
    visit(ui, "focus-source")
    # Start the key shortcut over a native page, with neither shell input selected.
    rect = ui.window.rectangle()
    mouse.click(coords=(rect.mid_point().x, rect.mid_point().y))
    observations = {}
    for method, marker in (("shortcut", "elixfocusone"), ("plus", "elixfocustwo")):
        previous = ui.session()
        if method == "shortcut":
            ui.keys("^t")
        else:
            ui.click("Ny fane", "Button")
        ui.wait(lambda: len(ui.session()["tabs"]) == len(previous["tabs"]) + 1
                and current_tab(ui).get("url") == "elix://start"
                and current_tab(ui).get("id") != previous["active"], "Ny fane ble ikke valgt")
        # Looking at the UIA focus bit and typing directly proves where typing lands.
        # Never click the edit, press Ctrl+L, or set its value during this check.
        edit = ui.find("Adressefelt")
        ui.wait(lambda: edit.element_info.element.CurrentHasKeyboardFocus, "Adressefeltet mangler fokus")
        ui.keys(marker)
        ui.wait(lambda: address_value(ui) == marker, "Tastene havnet utenfor adressefeltet")
        observations[method] = {"typed": address_value(ui), "tab_count": len(ui.session()["tabs"])}
        ui.keys("{ESC}")
    ui.screenshot("ELIX-17-new-tab-focus")
    return observations


def visible(ui, name, control_type):
    return any(item.is_visible() and item.rectangle().width() > 0 and item.rectangle().height() > 0
               for item in ui.window.descendants(title=name, control_type=control_type))


def gear_menu(ui):
    source = visit(ui, "gear-source")
    initial = len(ui.session()["tabs"])
    gear = "Innstillinger og hurtigvalg"
    ui.click(gear, "Button")
    ui.find("Lukk hurtigvalg", "Button")
    assert len(ui.session()["tabs"]) == initial
    assert current_tab(ui)["url"] == source
    for label in ("Delt visning", "Faner", "Arbeidsområde"):
        ui.find(label, "Button")
    ui.click("Delt visning", "Button")
    ui.find("Åpne delt visning", "Button")
    ui.click("Faner", "Button")
    ui.find("Søk i faner", "Button")
    ui.wait(lambda: not visible(ui, "Åpne delt visning", "Button"), "Forrige menyseksjon ble stående")
    ui.click("Arbeidsområde", "Button")
    ui.find("Arbeidsområde", "ComboBox")
    ui.screenshot("ELIX-29-quick-settings")
    ui.keys("{ESC}")
    ui.wait(lambda: not visible(ui, "Lukk hurtigvalg", "Button"), "Escape lukket ikke hurtigvalg")
    ui.find(gear, "Button").double_click_input()
    ui.wait(lambda: current_tab(ui).get("url") == "elix://settings", "Dobbeltklikk åpnet ikke innstillingene")
    ui.find("Farger og utseende", "Button")
    after = ui.session()
    assert len(after["tabs"]) == initial + 1
    assert any(tab["url"] == source for tab in after["tabs"])
    settings_id = after["active"]
    ui.find("ELIX gear-source", "TabItem").click_input()
    ui.wait(lambda: current_tab(ui).get("url") == source, "Kildesiden kunne ikke åpnes igjen")
    ui.find(gear, "Button").double_click_input()
    ui.wait(lambda: current_tab(ui).get("id") == settings_id, "Dobbeltklikk gjenbrukte ikke innstillingsfanen")
    assert len(ui.session()["tabs"]) == initial + 1
    return {"accordion_sections": 3, "single_click_tabs": initial, "double_click_tabs": initial + 1,
            "reused_existing_settings": True}


def icon_pixels(image):
    """Fixture swatch. The normal globe, spinner and theme contain no such fill."""
    return sum(red >= 235 and green <= 24 and 148 <= blue <= 193
               for red, green, blue in image.convert("RGB").getdata())


def shown_icon(ui, get_control):
    def sample():
        pic = get_control().capture_as_image()
        if pic is None:
            return False
        # The web icon is left of the row's label. Exclude text and adjacent rows.
        prefix = pic.crop((0, 0, min(pic.width, max(48, pic.height * 2)), pic.height))
        count = icon_pixels(prefix)
        return count if count >= 36 else False
    return ui.wait(sample, "Fant ikke testikonets magentafargede piksler i kontrollen", timeout=25)


def tab_favicon(ui):
    visit(ui, "tab-icon")
    get_tab = lambda: ui.find("ELIX tab-icon", "TabItem")
    active = shown_icon(ui, get_tab)
    ui.new_tab()
    ui.wait(lambda: current_tab(ui).get("url") == "elix://start", "Ny fane ble ikke valgt")
    background = shown_icon(ui, get_tab)
    get_tab().click_input()
    ui.wait(lambda: current_tab(ui).get("url") == ui.page("tab-icon"), "Kildesiden ble ikke valgt")
    again = shown_icon(ui, get_tab)
    ui.screenshot("ELIX-18-visible-tab-icon")
    return {"active_pixels": active, "background_pixels": background, "reactivated_pixels": again}


def bookmark_favicon(ui):
    slug = "bookmark-icon"
    title, url = "ELIX " + slug, visit(ui, slug)
    shown_icon(ui, lambda: ui.find(title, "TabItem"))
    ui.click("Legg til bokmerke", "Button")
    sql = ("SELECT favicon FROM bookmarks WHERE url=? AND "
           "profile=(SELECT value FROM settings WHERE key='active_profile')")
    def stored_icon():
        rows = query_db(ui, sql, (url,))
        return rows[0][0] if rows and rows[0][0] else False
    uri = ui.wait(stored_icon, "Bokmerkeikonet ble ikke lagret", timeout=25)
    assert uri.startswith("data:image/png;base64,")
    png = base64.b64decode(uri.split(",", 1)[1], validate=True)
    with Image.open(io.BytesIO(png)) as stored:
        stored_pixels = icon_pixels(stored)
    assert stored_pixels >= 36
    # The default bookmarks bar is scoped separately from tabs and page content.
    bar = lambda: ui.find("Bokmerker under adressefeltet")
    toolbar = shown_icon(ui, lambda: ui.find(title, "Button", root=bar()))
    ui.navigate("elix://bookmarks")
    ui.find("Valg for " + title, "Button", timeout=20)
    ui.restart()
    ui.find("Valg for " + title, "Button", timeout=35)
    def library_row():
        # On the bookmarks page the open button includes title and URL. The
        # toolbar button and the adjacent "Valg for …" action have other names.
        for row in ui.window.descendants(control_type="Button"):
            name = row.element_info.name or ""
            if row.is_visible() and name.startswith(title) and url in name:
                return row
        raise LookupError("Fant ikke bokmerket i den åpne bokmerkelisten")
    restored = shown_icon(ui, library_row)
    ui.screenshot("ELIX-21-restored-bookmark-icon")
    return {"saved_png_pixels": stored_pixels, "toolbar_pixels": toolbar, "restored_row_pixels": restored}


def search_query(url, text):
    try:
        parsed = urlsplit(url)
        return (parsed.scheme == "https" and parsed.hostname == "www.google.com" and
                parsed.path == "/search" and parse_qs(parsed.query).get("q") == [text])
    except (TypeError, ValueError):
        return False


def colon_search(ui):
    evidence = {}
    # The browser reserves "elix:" for internal navigation. Exercise both prose
    # and a search operator using queries from the production URL parser tests.
    for text in ("EKMP setning: her", "setning: her", "site:github.com Feil: #1"):
        ui.navigate(text)
        def actual():
            # Preserve the original destination if the external search provider
            # subsequently navigates to consent or another result page.
            shown = address_value(ui)
            if search_query(shown, text):
                return {"from": "address", "url": shown}
            rows = query_db(ui, "SELECT url FROM history WHERE profile=(SELECT value FROM settings "
                                "WHERE key='active_profile') ORDER BY id DESC LIMIT 12")
            return next(({"from": "history", "url": url} for (url,) in rows if search_query(url, text)), False)
        evidence[text] = ui.wait(actual, f"Mangler søkemålet for {text!r} i adressefelt og historikk", timeout=25)
    url = visit(ui, "port-in-address")
    ui.wait(lambda: address_value(ui) == url, "En adresse med eksplisitt port ble tolket som søk")
    ui.wait(lambda: query_db(ui, "SELECT 1 FROM history WHERE url=? AND profile=(SELECT value "
                                  "FROM settings WHERE key='active_profile')", (url,)),
            "Den besøkte fixture-adressen mangler i historikken")
    ui.screenshot("ELIX-33-literal-url")
    return {"searches": evidence, "literal_url": url}


def beats(ui, slug, since=0):
    return [event["time"] for event in ui.fixture.events
            if event.get("kind") == "heartbeat" and event.get("slug") == slug and event["time"] >= since]


def single_tab_sleep(ui):
    sleeper, peer, active = "sleep-target", "sleep-peer", "sleep-current"
    visit(ui, sleeper)
    ui.new_tab()
    visit(ui, peer)
    ui.new_tab()
    visit(ui, active)
    before = time.monotonic()
    ui.wait(lambda: all(beats(ui, slug, before) for slug in (sleeper, peer, active)),
            "Alle tre fanene må sende livstegn før dvaletesten", timeout=20)
    ui.click("Vis sidepanel", "Button")
    ui.click("Sett i dvale: ELIX " + sleeper, "Button")
    ui.find("Vekk fane: ELIX " + sleeper, "Button")
    # Allow a final in-flight heartbeat while the native view closes. Require an
    # observation interval as well as continued heartbeats from BOTH other pages.
    sleep_from = time.monotonic() + 1.0
    ui.wait(lambda: time.monotonic() >= sleep_from + 3.2 and
            len(beats(ui, peer, sleep_from)) >= 2 and len(beats(ui, active, sleep_from)) >= 2,
            "Andre faner stoppet da målfanen gikk i dvale", timeout=18)
    assert not beats(ui, sleeper, sleep_from), "Målfanen fortsatte å kjøre i dvale"
    independent = {slug: len(beats(ui, slug, sleep_from)) for slug in (peer, active)}
    ui.screenshot("ELIX-25-one-tab-asleep")
    wake = time.monotonic()
    ui.click("Vekk fane: ELIX " + sleeper, "Button")
    ui.wait(lambda: current_tab(ui).get("url") == ui.page(sleeper) and beats(ui, sleeper, wake),
            "Målfanen kom ikke tilbake etter vekking", timeout=25)
    return {"sleep_interval_seconds": round(wake - sleep_from, 2),
            "sleeping_heartbeats": 0, "other_heartbeats": independent,
            "woke_after_seconds": round(beats(ui, sleeper, wake)[0] - wake, 2)}


CASES = {"ELIX-17": new_tab_focus, "ELIX-18": tab_favicon, "ELIX-21": bookmark_favicon,
         "ELIX-25": single_tab_sleep, "ELIX-29": gear_menu, "ELIX-33": colon_search}
