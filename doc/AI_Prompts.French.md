# Commandes utilisées pour créer le programme

**Date :** 17 novembre 2026  
**LLM :** Claude Sonnet 5.5 (Claude Code, application de bureau)

*Note : l'assistant pouvait compiler, lancer les tests et prendre des captures de sa propre fenêtre, mais pas cliquer avec une vraie souris (sauf à la toute fin), donc certains comportements de l'interface n'ont été confirmés que par l'auteur. :-)*

---

## Prompt 1

> Nous allons travailler sur un nouveau projet en Rust 2024, dans le dossier Ycopy

* **Résultat :** `cargo new Ycopy --edition 2024` a créé le squelette, qui a compilé. L'exécution du binaire a été refusée par une stratégie de groupe Windows (erreur 1260), ce qui s'est avéré important par la suite.

---

## Prompt 2

> Crée l'application suivante, qui constituera le projet Ycopy.
>
> La fenêtre principale est une boîte de dialogue, avec les éléments suivants :
>
> * Icône représentant une flèche se dédoublant.
> * Champ dépôt : zone où l'on peut déposer des fichiers à copier
>    * Si l'on clique sur la zone, un sélecteur de fichier s'ouvre, permettant d'effectuer une sélection multiple
>    * Le champ est désactivé tant qu'au moins 2 destinations n'ont pas été choisies.
>    * Le champ contient une liste des fichiers qui ne sont pas encore traités
>
> * Champ destination1 : de type "nom de dossier". Un bouton "..." permet d'ouvrir un sélecteur de dossier, permettant de choisir la destination1.
> * Champ destination2 : de type "nom de dossier". Un bouton "..." permet d'ouvrir un sélecteur de dossier, permettant de choisir la destination2.
> * Bouton "Go!" : Lance la copie séquentielle (1 par 1) "des fichiers à copier" vers la destination1 et vers la destination2.
>    * La copie vers les 2 destinations ne doit engendrer qu'une seule lecture du fichier source, sans le copier intégralement en mémoire vive.
> * Une barre de statut indiquant : le temps de traitement restant, le débit de copie (en Mo/sec).
>
> Organiser le projet afin d'envisager l'internationalisation de l'interface, et l'utilisation du programme en ligne de commande. Pour commencer, prépare les traductions en Français, Anglais, Allemand, Espagnol, Italien et Portugais.
>
> Utilise les bibliothèques et frameworks adaptés à créer une application Windows avec le moins de dépendances possibles.
>
> Pose-moi des questions pour préciser des points mal exprimés si nécessaire.

* **Résultat :** L'assistant a posé trois questions au lieu de deviner (voir Prompt 3). Puis il a écrit tout le projet : un moteur de copie (`copier.rs`) qui lit par blocs de 4 Mio et écrit chaque bloc vers les deux destinations, une interface graphique (`gui.rs`), une interface en ligne de commande (`cli.rs`), un système de traduction sans dépendance qui lit `locales/*.txt`, et un générateur d'icône (`examples/make_icon.rs`). Rien n'a pu être compilé à ce stade à cause de la stratégie de groupe.

---

## Prompt 3

> *(Réponses aux questions de l'assistant, choisies dans une liste.)*
>
> Base de l'interface : `native-windows-gui`. Fichier déjà présent dans une destination : écraser. Ligne de commande : même exécutable, avec des arguments.

* **Résultat :** Ces choix ont déterminé la conception : une fine surcouche Win32 (peu de dépendances), l'écrasement silencieux, et un seul `ycopy.exe` qui ouvre la fenêtre quand aucun fichier n'est donné et copie sans fenêtre dans le cas contraire.

---

## Prompt 4

> J'autorise le dossier
>
> *(Puis, la compilation étant toujours bloquée :)* J'ai déplacé le projet dans C:\Claude\Ycopy

* **Résultat :** La première réponse n'a rien changé : une stratégie de groupe ne se lève pas par un message dans le chat, et l'assistant n'a pas essayé de la contourner. Déplacer le projet dans un dossier où l'exécution est autorisée a réglé le problème. Problème de l'œuf et de la poule à la première compilation : `build.rs` a besoin de l'icône, et le générateur d'icône est construit par le même projet, donc le générateur a été compilé une fois avec un simple appel à `rustc`. Deux petites erreurs de durée de vie (lifetimes) ont ensuite été corrigées.
* **Tests :** 12 tests unitaires réussis (moteur de copie, analyse de la ligne de commande, formatage, traductions). Un fichier de 200 Mo et un petit fichier ont été copiés vers deux dossiers en ligne de commande ; les empreintes SHA-256 des deux copies correspondaient à la source (environ 780 Mo/s sur cette machine). La fenêtre s'est ouverte, a répondu et s'est fermée correctement.

---

## Prompt 5

> Compile la livraison.

* **Résultat :** `cargo build --release` a produit `target\release\ycopy.exe` (environ 600 Ko, avec l'icône et les six langues intégrées).

---

## Prompt 6

> Est-ce que la langue est choisie automatiquement d'après la langue du système ?

* **Résultat :** Oui. Ordre de priorité : `--lang`, puis `YCOPY_LANG`, puis la langue de l'utilisateur Windows (format régional, pas la langue d'affichage), puis l'anglais. Seul `--lang` avait été testé à ce stade.

---

## Prompt 7

> Bien joué.
>
> Ajoute un sélecteur de langue en bas à droite de la barre de statut. Lors du changement de langue sélectionnée, l'interface se rafraîchit avec la nouvelle langue choisie. Ne PAS mémoriser la langue choisie pour les futures exécutions.

* **Résultat :** Une liste déroulante avec les six langues (affichées sous leur propre nom) a été ajoutée, et tous les textes sont réappliqués au changement, y compris la barre de statut, désormais conservée sous forme de données et réaffichée. Rien n'est écrit sur le disque. Le passage à l'allemand a été simulé avec des messages Win32 et les libellés ont bien changé. La compilation de livraison a dû attendre, car l'instance de l'auteur était ouverte et verrouillait le fichier.

---

## Prompt 8

> Le sélecteur ne fonctionne pas.
>
> Corrige, et prends en compte les modifications suivantes :
>
> * Tous les champs sont actifs dès le lancement, sauf le bouton Go.
> * Le bouton Go devient actif une fois destination1 et destination2 choisis.

* **Résultat :** Une capture de la fenêtre de l'application (prise avec `PrintWindow`) a montré la vraie cause : le sélecteur était entièrement caché derrière la barre de statut. Il est maintenant placé au-dessus de ses voisins et n'est plus recouvert. La zone de dépôt est toujours active (l'indication « choisissez d'abord les destinations » a été retirée des six langues) et Go suit l'état des destinations.
* **Tests :** Avec des messages envoyés à la fenêtre : Go est désactivé au démarrage, toujours désactivé avec une seule destination, activé avec deux destinations différentes, et désactivé de nouveau si les deux sont identiques. Le sélecteur est visible sur la nouvelle capture. Un vrai clic de souris sur la liste déroulante n'a pas été testé par l'assistant.

---

## Prompt 9

> Écris un fichier readme.md en anglais, destiné à publier ce projet sur GitHub
>
> Écris un fichier doc/AI_Prompts.md résumant cette session de travail, sur le modèle de celui-ci https://github.com/ju-git/icyfix/blob/main/doc/AI_Prompts.md

* **Résultat :** Ce fichier et `README.md`, ainsi qu'une capture de la fenêtre principale dans `doc/screenshot.png`. Aucune licence n'avait encore été choisie, le README le disait donc (voir Prompt 10).

---

## Prompt 10

> Oui, MIT
>
> *(Interrompu et corrigé :)* Non, licence GPLv3

* **Résultat :** La première réponse a été interrompue avant que quoi que ce soit ne soit écrit, MIT n'a donc jamais été appliquée. Le texte officiel de la GPL v3 a été téléchargé depuis gnu.org dans `LICENSE`, la section licence du README a été mise à jour, et `license = "GPL-3.0-only"` a été ajouté à `Cargo.toml`.

---

## Prompt 11

> Ajoute l'Ukrainien, avec l'affichage Українська (UKR)

* **Résultat :** Un fichier de langue `locales/uk.txt` (28 clés traduites), la variante `Lang::Uk` dans `src/i18n.rs` et l'entrée « Українська (UKR) » dans le sélecteur. Le sélecteur a été élargi de 120 à 150 px et sa liste rendue plus haute pour accueillir sept entrées. L'aide de `--lang` liste désormais `uk` dans toutes les langues, et les tests existants vérifient la parité des clés et des marqueurs du nouveau fichier. La traduction n'a pas été relue par un locuteur natif.

---

## Prompt 12

> Dans la liste des fichiers à traiter, ajoute sur chaque ligne une icône permettant de supprimer la ligne correspondante.

* **Résultat :** La liste de fichiers est maintenant une liste à deux colonnes au lieu d'une simple liste : le chemin, puis une marque « ✕ ». Un clic sur le ✕ retire le fichier de la liste ; un clic sur un chemin ou sur la zone vide ouvre toujours le sélecteur de fichiers ; la liste reste verrouillée pendant une copie.
* **Tests :** L'assistant a piloté la version de développement avec un vrai clic de souris : le sélecteur s'ouvre depuis la zone vide avec trois fichiers, la suppression de celui du milieu laisse les deux autres, un clic sur un chemin ouvre le sélecteur sans rien supprimer, et les lignes restantes se suppriment une à une. Le glisser-déposer sur la nouvelle liste et l'aspect du ✕ à l'écran n'ont pas été vérifiés.
