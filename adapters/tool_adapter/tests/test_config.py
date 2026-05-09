from __future__ import annotations

from pbsm_tool_adapter import ConfigManager, FormatType


def test_default_config():
    manager = ConfigManager()
    config = manager.get_config()
    assert "parser" in config
    assert "formats" in config
    assert "error_handling" in config


def test_get_parser_config():
    manager = ConfigManager()
    config = manager.get_config("parser")
    assert "max_parsing_time_ms" in config
    assert "default_confidence" in config


def test_get_formats_config():
    manager = ConfigManager()
    config = manager.get_config("formats")
    assert "json" in config
    assert "html" in config
    assert "text" in config
    assert "csv" in config
    assert "error" in config


def test_update_config():
    manager = ConfigManager()
    manager.update_config("parser", {"default_confidence": 0.9})
    config = manager.get_config("parser")
    assert config["default_confidence"] == 0.9


def test_update_config_no_merge():
    manager = ConfigManager()
    manager.update_config("parser", {"default_confidence": 0.9}, merge=False)
    config = manager.get_config("parser")
    assert "default_confidence" in config


def test_reset_config():
    manager = ConfigManager()
    manager.update_config("parser", {"default_confidence": 0.9})
    manager.reset_config("parser")
    config = manager.get_config("parser")
    assert config["default_confidence"] == 0.75


def test_validate_config_valid():
    manager = ConfigManager()
    config = manager.get_config()
    errors = manager.validate_config(config)
    assert errors == []


def test_validate_config_invalid_confidence():
    manager = ConfigManager()
    errors = manager.validate_config({"parser": {"default_confidence": 1.5}})
    assert len(errors) > 0


def test_validate_config_invalid_depth():
    manager = ConfigManager()
    errors = manager.validate_config({"formats": {"json": {"max_depth": 0}}})
    assert len(errors) > 0


def test_get_effective_config():
    manager = ConfigManager()
    config = manager.get_effective_config()
    assert "parser" in config
    assert "formats" in config


def test_get_effective_config_with_format():
    manager = ConfigManager()
    config = manager.get_effective_config(format_type=FormatType.JSON)
    assert "_format_specific" in config
    assert "max_depth" in config["_format_specific"]
