"""Tests for the .env file loader."""

from __future__ import annotations

import os

from agent_runtime.env_loader import load_dotenv


class TestLoadDotenv:
    def test_loads_simple_vars(self, tmp_path):
        env_file = tmp_path / ".env"
        env_file.write_text("TEST_KEY_A=hello\nTEST_KEY_B=world\n")

        # Ensure keys are not already set
        os.environ.pop("TEST_KEY_A", None)
        os.environ.pop("TEST_KEY_B", None)

        count = load_dotenv(env_file)
        assert count == 2
        assert os.environ["TEST_KEY_A"] == "hello"
        assert os.environ["TEST_KEY_B"] == "world"

        # Clean up
        os.environ.pop("TEST_KEY_A", None)
        os.environ.pop("TEST_KEY_B", None)

    def test_does_not_overwrite_existing(self, tmp_path):
        env_file = tmp_path / ".env"
        env_file.write_text("TEST_EXISTING_KEY=new_value\n")

        os.environ["TEST_EXISTING_KEY"] = "original"
        count = load_dotenv(env_file)
        assert count == 0
        assert os.environ["TEST_EXISTING_KEY"] == "original"

        os.environ.pop("TEST_EXISTING_KEY", None)

    def test_handles_quoted_values(self, tmp_path):
        env_file = tmp_path / ".env"
        env_file.write_text('TEST_DQUOTE="double quoted"\nTEST_SQUOTE=\'single quoted\'\n')

        os.environ.pop("TEST_DQUOTE", None)
        os.environ.pop("TEST_SQUOTE", None)

        load_dotenv(env_file)
        assert os.environ["TEST_DQUOTE"] == "double quoted"
        assert os.environ["TEST_SQUOTE"] == "single quoted"

        os.environ.pop("TEST_DQUOTE", None)
        os.environ.pop("TEST_SQUOTE", None)

    def test_skips_comments_and_blank_lines(self, tmp_path):
        env_file = tmp_path / ".env"
        env_file.write_text("# This is a comment\n\n  # Another comment\nTEST_ENV_VAR=value\n")

        os.environ.pop("TEST_ENV_VAR", None)

        count = load_dotenv(env_file)
        assert count == 1
        assert os.environ["TEST_ENV_VAR"] == "value"

        os.environ.pop("TEST_ENV_VAR", None)

    def test_nonexistent_file_returns_zero(self):
        count = load_dotenv("/nonexistent/path/.env")
        assert count == 0

    def test_llm_provider_env(self, tmp_path):
        """Test that LLM-specific env vars load correctly."""
        env_file = tmp_path / ".env"
        env_file.write_text(
            "LLM_PROVIDER=ollama\n"
            "LLM_MODEL=llama3\n"
            "OLLAMA_BASE_URL=http://localhost:11434\n"
        )

        for key in ("LLM_PROVIDER", "LLM_MODEL", "OLLAMA_BASE_URL"):
            os.environ.pop(key, None)

        count = load_dotenv(env_file)
        assert count == 3
        assert os.environ["LLM_PROVIDER"] == "ollama"
        assert os.environ["LLM_MODEL"] == "llama3"
        assert os.environ["OLLAMA_BASE_URL"] == "http://localhost:11434"

        for key in ("LLM_PROVIDER", "LLM_MODEL", "OLLAMA_BASE_URL"):
            os.environ.pop(key, None)
