"""Shared pieces of the mockups: page size, the app's strings, icons, logo."""
from pathlib import Path

HERE = Path(__file__).resolve().parent
W, H = 1280, 800


# Mockup text comes from the app's own strings; data-i18n lets scripts/i18n-editor.py swap it live.
def load(lang):
    path = HERE.parent.parent / f"assets/i18n/{lang}.lang"
    return {
        k.strip(): v.strip()
        for k, v in (
            line.split("=", 1)
            for line in path.read_text(encoding="utf-8").splitlines()
            if "=" in line and not line.lstrip().startswith("#")
        )
        if v.strip()
    }


EN = load("en")
LANGS = {"en": EN, "uk": {**EN, **load("uk")}}
STRINGS = EN  # the language being rendered; build.py switches it


def T(key, **sample):
    text = STRINGS[key]
    for name, value in sample.items():
        text = text.replace("{" + name + "}", str(value))
    return f'<span data-i18n="{key}">{text}</span>'


def tip(key):
    return f'data-i18n-title="{key}" title="{STRINGS[key]}"'


LOGO = """<svg class="logo" viewBox="0 0 32 24" aria-label="Glint"><rect x="1" y="4" width="30" height="16" rx="4" fill="#4dff9b" stroke="none"/><rect x="6.5" y="6.5" width="19" height="11" rx="1" fill="#07130c" stroke="none"/><path d="M12.8 8v8M19.2 8v8" stroke="#4dff9b" stroke-opacity=".3" stroke-width="1"/><path d="M3.5 9v6M28.5 9v6" stroke="#00140a" stroke-width="1"/><path d="M14.1 1v22" stroke="#fff" stroke-width="1.5"/></svg>"""
LIGHTS = '<div class="lights"><i></i><i></i><i></i></div>'


def ph(name):
    return f'<i class="ph ph-{name}"></i>'

