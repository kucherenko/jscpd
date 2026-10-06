from enum import Enum


class Color(Enum):
    """The colors a theme can pick from."""

    RED = 1
    ORANGE = 2
    YELLOW = 3
    GREEN = 4
    CYAN = 5
    BLUE = 6
    INDIGO = 7
    VIOLET = 8
    WHITE = 9
    GRAY = 10
    BLACK = 11
    BROWN = 12
    PINK = 13
    GOLD = 14
    SILVER = 15
    OLIVE = 16


THEME_PAGES = [
    page("palette", views.palette, title="Palette"),
    page("contrast", views.contrast, title="Contrast check"),
    page("fonts", views.fonts, title="Fonts"),
    page("preview", views.preview, title="Preview"),
    page("export", views.export_theme, title="Export"),
]
