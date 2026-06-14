```markdown
# agent-world Development Patterns

> Auto-generated skill from repository analysis

## Overview
This skill teaches the core development patterns and conventions used in the `agent-world` Python codebase. You'll learn about file naming, import/export styles, commit conventions, and how to write and run tests. The repository uses clear, maintainable Python practices with a focus on conventional commit messages and modular code organization.

## Coding Conventions

### File Naming
- All files use **snake_case**.
  - Example: `agent_manager.py`, `utils_helper.py`

### Import Style
- **Relative imports** are preferred within the package.
  - Example:
    ```python
    from .utils_helper import some_function
    ```

### Export Style
- **Named exports** are used; modules explicitly define what they export.
  - Example:
    ```python
    __all__ = ['Agent', 'AgentManager']
    ```

### Commit Messages
- Follows the **conventional commit** pattern.
- Prefixes like `fix` are used.
- Example:
  ```
  fix: resolve agent initialization bug in agent_manager.py
  ```

## Workflows

### Making a Code Change
**Trigger:** When you need to fix a bug or add a feature  
**Command:** `/code-change`

1. Create or update a Python file using snake_case naming.
2. Use relative imports for referencing other modules.
3. Make your code changes.
4. Add or update named exports as needed.
5. Write or update tests in a corresponding `*.test.*` file.
6. Commit your changes using a conventional commit message, e.g.:
   ```
   fix: update agent decision logic for edge cases
   ```
7. Push your changes to the repository.

### Writing and Running Tests
**Trigger:** When you add new functionality or fix a bug  
**Command:** `/run-tests`

1. Create a test file matching the pattern `*.test.*` (e.g., `agent_manager.test.py`).
2. Write test functions for your new or changed code.
3. Use the project's preferred (unspecified) testing framework.
4. Run all test files to ensure correctness.

## Testing Patterns

- Test files are named using the pattern `*.test.*` (e.g., `agent_manager.test.py`).
- The specific test framework is not defined, so follow standard Python test practices (e.g., using `unittest` or `pytest`).
- Place test files alongside or near the modules they test.

**Example:**
```python
# agent_manager.test.py

def test_agent_initialization():
    agent = Agent()
    assert agent.is_initialized
```

## Commands
| Command        | Purpose                                         |
|----------------|-------------------------------------------------|
| /code-change   | Guide for making code changes and committing    |
| /run-tests     | Instructions for writing and running tests      |
```
