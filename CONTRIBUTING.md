
---

# 🤝 Contributing to the Project

By contributing, you agree that your work will be licensed under the **PolyForm Noncommercial License 1.0.0**.

## 📖 Terms of Contribution

As per the [PolyForm Noncommercial License](https://www.google.com/search?q=LICENSE), the following applies to all contributions:

- **Copyright License:** The licensor grants a license to do everything that would otherwise infringe copyright for permitted purposes.

- **Changes & New Works:** You are granted a license to make changes and new works based on the software for noncommercial purposes.

- **Distribution:** You may distribute copies of the software, including your changes, provided you include these terms.


## 🏗️ Architectural Constraints

- **Logic in Libs:** All functional logic belongs in `libs/` (`win32_lib`, `wrapper_lib`, or `hybrid_lib`).

- **Main Wrapper:** The `main` package only calls the run function.

- **Minimal Defaults:** We add very few defaults to keep the code clean.

## 🛠️ Development Process

1. Fork the repository.

2. Create a feature branch.

3. Ensure your `Cargo.toml` updates are reflected in the `--license` output.

4. Submit a Pull Request with a clear description of changes.

## ⚠️ Patent & Copyright

You grant a non-exclusive, royalty-free license for your contributions to be used, modified, and distributed under the project's noncommercial terms.

## ⚖️ Patent Defense

If you make a written claim that the software infringes any patent, your patent license for the software ends immediately.

---
