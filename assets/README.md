# Levelog visual assets

`levelog-icon.png` is the 1254×1254 RGBA source used to generate the Tauri icon set in `src-tauri/icons/`.

- Created for this repository on 2026-07-20 with OpenAI's built-in image generation tool.
- Prompt direction: a production macOS squircle where three connected evidence nodes and layered facets form an upward growth trajectory; deep navy, cyan/blue, one emerald confirmation node, one gold future point; no text, letters, shield, trophy, watermark, external logo, or photorealistic device mockup.
- The generated flat magenta exterior was removed locally to create transparent corners. The subject was validated at source size and 32×32 before running `pnpm tauri icon assets/levelog-icon.png`.
- The asset and its generated size variants are distributed with Levelog under the repository's MIT License.

When replacing the icon, keep the source square with an alpha channel, verify it at 32×32, regenerate every platform variant with the command above, and document the new asset provenance here.
