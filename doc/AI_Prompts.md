# Commands Used to Create the Program

**Date:** November 17, 2026  
**LLM:** Claude Sonnet 5.5 (Claude Code, desktop app)

*Note: the prompts were written in French; they are translated here. The assistant could compile, run the tests and take screenshots of its own window, but not click with a real mouse, so some UI behaviour was only confirmed by the author. :-)*

---

## Prompt 1

> We are going to work on a new project in Rust 2024, in the folder Ycopy.

* **Result:** `cargo new Ycopy --edition 2024` created the skeleton and it compiled. Running the binary was refused by a Windows group policy (error 1260), which turned out to matter later.

---

## Prompt 2

> Create the following application, which will make up the Ycopy project.
>
> The main window is a dialog box with these elements:
>
> * An icon showing an arrow splitting in two.
> * A drop field: an area where files to copy can be dropped. Clicking it opens a file picker with multiple selection. The field is disabled until at least 2 destinations have been chosen. It lists the files that have not been processed yet.
> * Field destination1: a folder-name field, with a "..." button that opens a folder picker.
> * Field destination2: same as destination1.
> * A "Go!" button: starts the sequential (one by one) copy of the files to destination1 and destination2. The copy to both destinations must read the source file only once, without copying it entirely into RAM.
> * A status bar showing the remaining time and the copy speed (in MB/s).
>
> Organise the project so that the interface can be internationalised and the program can be used from the command line. To begin with, prepare the translations in French, English, German, Spanish, Italian and Portuguese.
>
> Use the libraries and frameworks best suited to a Windows application with as few dependencies as possible.
>
> Ask me questions if some points are unclear.

* **Result:** The assistant asked three questions instead of guessing (see Prompt 3). Then it wrote the whole project: a copy engine (`copier.rs`) reading 4 MiB chunks and writing each one to both destinations, a GUI (`gui.rs`), a command-line front-end (`cli.rs`), a dependency-free translation system reading `locales/*.txt`, and a generator for the icon (`examples/make_icon.rs`). Nothing could be compiled at that point because of the group policy.

---

## Prompt 3

> *(Answers to the assistant's questions, chosen from a list.)*
>
> GUI base: `native-windows-gui`. Existing file in a destination: overwrite. Command line: same executable, with arguments.

* **Result:** Those choices shaped the design: a thin Win32 wrapper (few dependencies), silent overwrite, and one `ycopy.exe` that opens the window when no file is given and copies without a window otherwise.

---

## Prompt 4

> I authorize the folder.
>
> *(Then, after the build was still blocked:)* I moved the project to `C:\Claude\Ycopy`.

* **Result:** The first answer did not change anything: a group policy cannot be lifted by a chat message, and the assistant did not try to work around it. Moving the project to a folder where execution is allowed solved it. Chicken-and-egg problem on the first build: `build.rs` needs the icon, and the icon generator is built by the same crate, so the generator was compiled once with a plain `rustc` call. Two small lifetime errors were then fixed.
* **Tests:** 12 unit tests passed (copy engine, command-line parsing, formatting, translations). A 200 MB file and a small file were copied to two folders from the command line; the SHA-256 hashes of both copies matched the source (about 780 MB/s on this machine). The window opened, responded, and closed properly.

---

## Prompt 5

> Compile the delivery.

* **Result:** `cargo build --release` produced `target\release\ycopy.exe` (about 600 KB, with the icon and the six languages embedded).

---

## Prompt 6

> Is the language chosen automatically from the system language?

* **Result:** Yes. Priority: `--lang`, then `YCOPY_LANG`, then the Windows user locale (regional format, not the display language), then English. Only `--lang` had been tested at that stage.

---

## Prompt 7

> Well done.
>
> Add a language selector at the bottom right of the status bar. When the selected language changes, the interface refreshes with the new language. Do NOT remember the chosen language for future runs.

* **Result:** A drop-down with the six languages (shown in their own names) was added, and all the texts are re-applied on change, including the status bar, which is now stored as data and re-rendered. Nothing is written to disk. Switching to German was simulated with Win32 messages and the labels changed correctly. The release build had to wait because the author's own instance was running and locking the file.

---

## Prompt 8

> The selector does not work.
>
> Fix it, and take into account the following changes:
>
> * All fields are active from the start, except the Go button.
> * The Go button becomes active once destination1 and destination2 are chosen.

* **Result:** A screenshot of the app's own window (taken with `PrintWindow`) showed the real cause: the selector was completely hidden behind the status bar. It is now raised above its siblings and no longer painted over. The drop zone is always enabled (the "choose the destinations first" hint was removed from the six languages) and Go follows the destinations.
* **Tests:** With messages sent to the window: Go is disabled at start, still disabled with one destination, enabled with two different ones, and disabled again when both are identical. The selector is visible in the new screenshot. A real mouse click on the drop-down was not tested by the assistant.

---

## Prompt 9

> Write a readme.md file in English, intended to publish this project on GitHub.
>
> Write a file doc/AI_Prompts.md summarising this work session, following the model of this one: https://github.com/ju-git/icyfix/blob/main/doc/AI_Prompts.md

* **Result:** This file and `README.md`, plus a screenshot of the main window in `doc/screenshot.png`. No license had been chosen yet, so the README said so (see Prompt 10).

---

## Prompt 10

> Yes, MIT.
>
> *(Interrupted and corrected:)* No, GPLv3 license.

* **Result:** The first answer was interrupted before anything was written, so MIT was never applied. The official GPL v3 text was downloaded from gnu.org into `LICENSE`, the README license section was updated, and `license = "GPL-3.0-only"` was added to `Cargo.toml`.
