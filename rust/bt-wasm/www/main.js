import init, { WasmGame } from '../pkg/bt_wasm.js';

// Game state
let game = null;
let lastFrameTime = 0;
let paused = false;

// Canvas and context
const canvas = document.getElementById('gameCanvas');
const ctx = canvas.getContext('2d');

// UI elements
const scoreValue = document.getElementById('scoreValue');
const linesValue = document.getElementById('linesValue');
const fundsValue = document.getElementById('fundsValue');
const linesToBazaarValue = document.getElementById('linesToBazaarValue');
const gameOverOverlay = document.getElementById('gameOverOverlay');
const newGameBtn = document.getElementById('newGameBtn');

// Palette: cell id -> { bright, dark }
const PALETTE = {
    1: { bright: '#fffff0', dark: '#808080' }, // IVORY
    2: { bright: '#ffff00', dark: '#b8860b' }, // YELLOW
    3: { bright: '#ff0000', dark: '#8b0000' }, // RED
    4: { bright: '#3050ff', dark: '#00008b' }, // BLUE
    5: { bright: '#ffa500', dark: '#b25900' }, // ORANGE
    6: { bright: '#00d000', dark: '#006400' }, // GREEN
    7: { bright: '#00ffff', dark: '#008b8b' }, // CYAN
    8: { bright: '#a020f0', dark: '#551a8b' }, // PURPLE
    9: { bright: '#bebebe', dark: '#bebebe' }, // NEUTRAL
    20: { bright: '#bebebe', dark: '#bebebe' }, // BOTTLE-NECK STRUCT
    23: { bright: '#ff00ff', dark: '#800080' }, // GIMP
};

const CELL_SIZE = 23;
const BEVEL_BORDER = 3;

// Initialize the game
async function initGame() {
    await init();
    const seed = performance.now() | 0;
    game = new WasmGame(seed);

    // Set canvas size based on game dimensions
    const width = game.width();
    const height = game.height();
    canvas.width = width * CELL_SIZE;
    canvas.height = height * CELL_SIZE;

    // Set CSS for scaling (1.6x)
    canvas.style.width = (width * CELL_SIZE * 1.6) + 'px';
    canvas.style.height = (height * CELL_SIZE * 1.6) + 'px';

    paused = false;
    gameOverOverlay.style.display = 'none';
    lastFrameTime = performance.now();
}

function newGame() {
    const seed = performance.now() | 0;
    game = new WasmGame(seed);
    paused = false;
    gameOverOverlay.style.display = 'none';
    lastFrameTime = performance.now();
}

function drawCell(x, y, cellId) {
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
        ctx.fillStyle = colors.dark;
        ctx.fillRect(px, py, CELL_SIZE, CELL_SIZE);
        // Bright inset on top-left
        ctx.fillStyle = colors.bright;
        ctx.fillRect(px, py, CELL_SIZE - BEVEL_BORDER, CELL_SIZE - BEVEL_BORDER);
        return;
    }

    // NEUTRAL / BOTTLE-NECK (9 or 20)
    if (cellId === 9 || cellId === 20) {
        ctx.fillStyle = '#bebebe';
        ctx.fillRect(px, py, CELL_SIZE, CELL_SIZE);
        return;
    }

    // GIMP (23)
    if (cellId === 23) {
        ctx.fillStyle = '#800080';
        ctx.fillRect(px, py, CELL_SIZE, CELL_SIZE);
        ctx.fillStyle = '#ff00ff';
        ctx.fillRect(px, py, CELL_SIZE - BEVEL_BORDER, CELL_SIZE - BEVEL_BORDER);
        return;
    }

    // HAPPY (21) and UNHAPPY (22)
    if (cellId === 21 || cellId === 22) {
        // Beveled yellow box
        ctx.fillStyle = '#b8860b';
        ctx.fillRect(px, py, CELL_SIZE, CELL_SIZE);
        ctx.fillStyle = '#ffff00';
        ctx.fillRect(px, py, CELL_SIZE - BEVEL_BORDER, CELL_SIZE - BEVEL_BORDER);

        // Draw face
        ctx.fillStyle = '#000000';

        // Eyes: two ellipses
        const eyeWidth = 4;
        const eyeHeight = 7;
        const eyeY = py + 5;

        // Left eye
        ctx.beginPath();
        ctx.ellipse(px + 4, eyeY, eyeWidth / 2, eyeHeight / 2, 0, 0, Math.PI * 2);
        ctx.fill();

        // Right eye
        ctx.beginPath();
        ctx.ellipse(px + 13, eyeY, eyeWidth / 2, eyeHeight / 2, 0, 0, Math.PI * 2);
        ctx.fill();

        // Mouth
        if (cellId === 21) {
            // Happy: smile (lower half of arc)
            ctx.beginPath();
            ctx.arc(px + 11.5, py + 12, 5, 0, Math.PI);
            ctx.stroke();
        } else {
            // Unhappy: frown (upper half of arc)
            ctx.beginPath();
            ctx.arc(px + 11.5, py + 12, 5, Math.PI, 0);
            ctx.stroke();

            // Tear: small blue dot below right eye
            ctx.fillStyle = '#3050ff';
            ctx.beginPath();
            ctx.arc(px + 13, py + 8, 2, 0, Math.PI * 2);
            ctx.fill();
        }
        return;
    }

    // DICE (24-29)
    if (cellId >= 24 && cellId <= 29) {
        // Beveled ivory box
        ctx.fillStyle = '#808080';
        ctx.fillRect(px, py, CELL_SIZE, CELL_SIZE);
        ctx.fillStyle = '#fffff0';
        ctx.fillRect(px, py, CELL_SIZE - BEVEL_BORDER, CELL_SIZE - BEVEL_BORDER);

        // Draw pips
        const value = cellId - 23;
        const X = [1, 7, 13];
        const Y = [1, 7, 13];
        const pipSize = 5;

        ctx.fillStyle = '#000000';

        const drawPip = (offsetX, offsetY) => {
            ctx.fillRect(px + offsetX, py + offsetY, pipSize, pipSize);
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

function render() {
    // Clear canvas with black background
    ctx.fillStyle = '#000000';
    ctx.fillRect(0, 0, canvas.width, canvas.height);

    // Get grid data
    const grid = game.render_grid();
    const width = game.width();
    const height = game.height();

    // Draw each cell
    for (let y = 0; y < height; y++) {
        for (let x = 0; x < width; x++) {
            const cellId = grid[y * width + x];
            drawCell(x, y, cellId);
        }
    }

    // Drain events (for now just consume them)
    game.drain_events();
}

function updateStats() {
    scoreValue.textContent = game.score();
    linesValue.textContent = game.lines();
    fundsValue.textContent = game.funds();
    const linesToBazaar = 20 - (game.lines() % 20);
    linesToBazaarValue.textContent = linesToBazaar;
}

function gameLoop(now) {
    if (lastFrameTime === 0) {
        lastFrameTime = now;
    }

    const dt = Math.min(now - lastFrameTime, 100);
    lastFrameTime = now;

    // Advance game if not paused
    if (!paused && !game.is_game_over()) {
        game.tick(dt);
    }

    // Render
    render();
    updateStats();

    // Show game over overlay if needed
    if (game.is_game_over()) {
        gameOverOverlay.style.display = 'flex';
    }

    requestAnimationFrame(gameLoop);
}

// Input handling
function handleKeyDown(e) {
    if (!game) return;

    switch (e.key) {
        case 'ArrowLeft':
            e.preventDefault();
            game.move_left();
            break;
        case 'ArrowRight':
            e.preventDefault();
            game.move_right();
            break;
        case 'ArrowUp':
            e.preventDefault();
            game.rotate();
            break;
        case 'ArrowDown':
            e.preventDefault();
            game.begin_drop();
            break;
        case 'p':
        case 'P':
            paused = !paused;
            game.set_paused(paused);
            break;
    }
}

// Event listeners
document.addEventListener('keydown', handleKeyDown);
newGameBtn.addEventListener('click', newGame);

// Initialize and start game loop
(async () => {
    await initGame();
    requestAnimationFrame(gameLoop);
})();
