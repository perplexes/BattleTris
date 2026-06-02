// Shared board rendering for the live game (main.js) and the replay player
// (replay.js). Keeping a single draw path guarantees playback looks pixel-for-
// pixel identical to the original game. Faithful to BTBox.C.

// Preload gimp image for cell id 23 (resolved relative to the page; both the
// game and the replay page live under /www/).
export const gimpImg = new Image();
gimpImg.src = 'assets/btgimp.png';

// Palette: cell id -> { bright, dark }. Exact RGB from the original X11
// resource defaults (BattleTris.C): bright = base color, dark = its
// dark/shadow variant used for the bevel border.
export const PALETTE = {
    1: { bright: '#eeeee0', dark: '#a8a8a8' }, // IVORY  / GRAY
    2: { bright: '#eeee00', dark: '#daa520' }, // YELLOW / dark (goldenrod)
    3: { bright: '#ee0000', dark: '#8b0000' }, // RED    / dark red
    4: { bright: '#0000cd', dark: '#00008b' }, // BLUE   / dark blue
    5: { bright: '#ee9a00', dark: '#da7600' }, // ORANGE / dark orange
    6: { bright: '#32cd32', dark: '#228b22' }, // GREEN  / forest green
    7: { bright: '#009acd', dark: '#436eee' }, // CYAN   (a deep blue!) / variant
    8: { bright: '#a020f0', dark: '#68228b' }, // PURPLE / dark purple
    9: { bright: '#bfbfbf', dark: '#bfbfbf' }, // NEUTRAL
    20: { bright: '#bfbfbf', dark: '#bfbfbf' }, // BOTTLE-NECK STRUCT
    23: { bright: '#ff00ff', dark: '#800080' }, // GIMP (placeholder; orig is an image)
};

export const CELL_SIZE = 23;
export const BEVEL_BORDER = 3;

export function drawBoard(context, grid, width, height) {
    // Clear canvas with black background
    context.fillStyle = '#000000';
    context.fillRect(0, 0, width * CELL_SIZE, height * CELL_SIZE);

    // Draw each cell
    for (let y = 0; y < height; y++) {
        for (let x = 0; x < width; x++) {
            const cellId = grid[y * width + x];
            drawCellOnContext(context, x, y, cellId);
        }
    }
}

export function drawCellOnContext(context, x, y, cellId) {
    const px = x * CELL_SIZE;
    const py = y * CELL_SIZE;

    // Empty or hidden cells: draw nothing (black background)
    if (cellId <= 0) {
        return;
    }

    // Beveled colored boxes (1-8)
    if (cellId >= 1 && cellId <= 8) {
        const colors = PALETTE[cellId];
        // Dark shadow on bottom-right
        context.fillStyle = colors.dark;
        context.fillRect(px, py, CELL_SIZE, CELL_SIZE);
        // Bright inset on top-left
        context.fillStyle = colors.bright;
        context.fillRect(px, py, CELL_SIZE - BEVEL_BORDER, CELL_SIZE - BEVEL_BORDER);
        return;
    }

    // NEUTRAL / BOTTLE-NECK (9 or 20)
    if (cellId === 9 || cellId === 20) {
        context.fillStyle = '#bebebe';
        context.fillRect(px, py, CELL_SIZE, CELL_SIZE);
        return;
    }

    // GIMP (23): draw image if loaded, else magenta bevel placeholder
    if (cellId === 23) {
        if (gimpImg.complete && gimpImg.naturalWidth > 0) {
            context.drawImage(gimpImg, px, py, CELL_SIZE, CELL_SIZE);
        } else {
            context.fillStyle = '#800080';
            context.fillRect(px, py, CELL_SIZE, CELL_SIZE);
            context.fillStyle = '#ff00ff';
            context.fillRect(px, py, CELL_SIZE - BEVEL_BORDER, CELL_SIZE - BEVEL_BORDER);
        }
        return;
    }

    // HAPPY (21) and UNHAPPY (22)
    if (cellId === 21 || cellId === 22) {
        // Beveled yellow box (goldenrod shadow, yellow face) — as BTBox.C.
        context.fillStyle = '#daa520';
        context.fillRect(px, py, CELL_SIZE, CELL_SIZE);
        context.fillStyle = '#eeee00';
        context.fillRect(px, py, CELL_SIZE - BEVEL_BORDER, CELL_SIZE - BEVEL_BORDER);

        // Draw face
        context.fillStyle = '#000000';

        // Eyes: two ellipses
        const eyeWidth = 4;
        const eyeHeight = 7;
        const eyeY = py + 5;

        // Left eye
        context.beginPath();
        context.ellipse(px + 4, eyeY, eyeWidth / 2, eyeHeight / 2, 0, 0, Math.PI * 2);
        context.fill();

        // Right eye
        context.beginPath();
        context.ellipse(px + 13, eyeY, eyeWidth / 2, eyeHeight / 2, 0, 0, Math.PI * 2);
        context.fill();

        // Mouth
        if (cellId === 21) {
            // Happy: smile (lower half of arc)
            context.beginPath();
            context.arc(px + 11.5, py + 12, 5, 0, Math.PI);
            context.stroke();
        } else {
            // Unhappy: frown (upper half of arc)
            context.beginPath();
            context.arc(px + 11.5, py + 12, 5, Math.PI, 0);
            context.stroke();

            // Tear: small blue dot below right eye
            context.fillStyle = '#3050ff';
            context.beginPath();
            context.arc(px + 13, py + 8, 2, 0, Math.PI * 2);
            context.fill();
        }
        return;
    }

    // DICE (24-29)
    if (cellId >= 24 && cellId <= 29) {
        // Beveled ivory box (gray shadow, ivory face) — as BTBox.C die boxes.
        context.fillStyle = '#a8a8a8';
        context.fillRect(px, py, CELL_SIZE, CELL_SIZE);
        context.fillStyle = '#eeeee0';
        context.fillRect(px, py, CELL_SIZE - BEVEL_BORDER, CELL_SIZE - BEVEL_BORDER);

        // Draw pips: a 5x5 gray square with a 3x3 black inset (BTBox.C).
        const value = cellId - 23;
        const X = [1, 7, 13];
        const Y = [1, 7, 13];
        const pipSize = 5;

        const drawPip = (offsetX, offsetY) => {
            context.fillStyle = '#a8a8a8';
            context.fillRect(px + offsetX, py + offsetY, pipSize, pipSize);
            context.fillStyle = '#000000';
            context.fillRect(px + offsetX + 1, py + offsetY + 1, pipSize - 2, pipSize - 2);
        };

        // Pip placement rules
        if (value > 1) {
            drawPip(X[0], Y[0]); // TL
            drawPip(X[2], Y[2]); // BR
        }
        if (value > 3) {
            drawPip(X[2], Y[0]); // TR
            drawPip(X[0], Y[2]); // BL
        }
        if (value % 2 === 1) {
            drawPip(X[1], Y[1]); // Center
        }
        if (value === 6) {
            drawPip(X[0], Y[1]); // ML
            drawPip(X[2], Y[1]); // MR
        }
        return;
    }
}
