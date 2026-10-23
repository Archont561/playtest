from playtest import VERSION, __version__


def test_version_accessible():
    assert VERSION == "0.1.0"
    assert __version__ == "0.1.0"
