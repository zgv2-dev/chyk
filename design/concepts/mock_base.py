"""Shared pieces of the mockups: page size, the app's strings, icons, key caps, logo."""
from pathlib import Path

HERE = Path(__file__).resolve().parent
W, H = 1280, 800
FILE = "rooftop_take3.mov"


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


def T(key, new=None, **sample):
    """The app's string `key`. Proposed strings that aren't in the .lang files yet pass their
    English as `new`, which every language shows until they're added (like the app's fallback)."""
    text = STRINGS.get(key, new)
    if text is None:
        raise KeyError(key)
    for name, value in sample.items():
        text = text.replace("{" + name + "}", str(value))
    return f'<span data-i18n="{key}">{text}</span>'


def tip(key, new=None):
    text = STRINGS.get(key, new)
    return f'data-i18n-title="{key}" title="{text}"'


def ph(name, fill=False):
    return f'<i class="ph{"-fill" if fill else ""} ph-{name}"></i>'


def kbd(*keys):
    """Key caps for a shortcut, e.g. kbd("⌘", "K")."""
    return '<span class="keys">' + "".join(f"<kbd>{k}</kbd>" for k in keys) + "</span>"


def tooltip(anchor, label, *keys, side="top"):
    """A tooltip pinned to the element matching CSS selector `anchor` (placed by build.PLACE)."""
    return (f'<div class="tooltip {side}" data-for="{anchor}"><span>{label}</span>'
            f'{kbd(*keys) if keys else ""}</div>')


LOGO = '<img class="logo" src="assets/mark.png" alt="Chyk">'
LIGHTS = '<div class="lights"><i></i><i></i><i></i></div>'
