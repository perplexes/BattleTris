// motif-dialog.ts — a period-authentic OSF/Motif message dialog (an "OK" modal).
//
// This recreates the ORIGINAL game's BTMessageDlog (usr/src/widget/BTMessageDlog.C),
// which is an Xm/Motif `XmCreateWarningDialog`:
//     XmNdialogStyle      = XmDIALOG_FULL_APPLICATION_MODAL   (fully modal)
//     XmNdefaultButtonType= XmDIALOG_OK_BUTTON                (OK is the default)
//     the HELP + CANCEL buttons are unmanaged                 (only OK remains)
// A Motif WarningDialog draws a warning symbol (an exclamation in a triangle) to the
// LEFT of the message, the message string, then a single default OK push button.
//
// Colors/fonts are the literal resources from usr/src/share/BattleTris.ad:
//     BTMessageDlog*background            : gray75  (#BFBFBF)
//     BTMessageDlog*foreground            : red3    (#CD0000)  — the message is RED
//     BTMessageDlog*XmPushButton*foreground: blue   (#0000FF)  — the OK label is BLUE
//     BTMessageDlog*XmPushButton*background: gray    (#BEBEBE)
//     BTMessageDlog*XmString*fontList     : helvetica-bold-14  — the message font
// Bevels follow the project's existing Motif palette (motif.css / style.css): a hard
// white/light highlight top-left + black/dark shadow bottom-right, no rounded corners,
// no gradients — crisp, non-upscaled 1994 pixels.
//
// The body is intentionally a free DOM slot (string OR an arbitrary HTMLElement) so a
// later, richer widget — the planned DDR-style minigame for the UPDATE button — can
// live inside the same beveled frame without reworking this shell.

/** What goes in the dialog body: a plain message, or arbitrary DOM (the future seam). */
export type MotifDialogContent = string | Node;

export interface MotifDialogOptions {
    /** Title-bar text. Defaults to "BattleTris". */
    title?: string;
    /** The single button's label. Defaults to "OK" (the Motif default button). */
    buttonLabel?: string;
    /** Show the Motif warning symbol (exclamation-in-triangle) left of the body. Default true. */
    symbol?: boolean;
}

// Only ONE dialog at a time (idempotent): a second call while one is open resolves
// immediately rather than stacking overlays. Tracks the live root + how to dismiss it.
let activeRoot: HTMLElement | null = null;
let activeClose: (() => void) | null = null;

/**
 * Show a modal Motif "OK" dialog. Resolves when the user dismisses it (OK / Enter / Esc).
 *
 * Modal like the original's XmDIALOG_FULL_APPLICATION_MODAL: a dimmed backdrop, focus
 * pinned to the dialog (Tab is trapped on the OK button), Enter and Esc both fire OK,
 * and the OK button is focused on open (showing the default-button focus rectangle).
 */
export function showMotifDialog(content: MotifDialogContent, opts: MotifDialogOptions = {}): Promise<void> {
    // Idempotent: never stack two dialogs.
    if (activeRoot) return Promise.resolve();

    const title = opts.title ?? 'BattleTris';
    const buttonLabel = opts.buttonLabel ?? 'OK';
    const showSymbol = opts.symbol !== false;

    return new Promise<void>((resolve) => {
        const prevFocus = document.activeElement as HTMLElement | null;

        // ── Backdrop (the modal scrim) ─────────────────────────────────────────
        const overlay = document.createElement('div');
        overlay.className = 'motif-dialog-overlay';
        overlay.setAttribute('role', 'dialog');
        overlay.setAttribute('aria-modal', 'true');

        // ── The beveled dialog window ──────────────────────────────────────────
        const dialog = document.createElement('div');
        dialog.className = 'motif-dialog';

        // Title bar (beveled, blue label — the project's title-label convention).
        const titlebar = document.createElement('div');
        titlebar.className = 'motif-dialog-titlebar';
        titlebar.textContent = title;
        dialog.appendChild(titlebar);
        overlay.setAttribute('aria-label', title);

        // Message row: warning symbol (left) + body (right), like a Motif WarningDialog.
        const row = document.createElement('div');
        row.className = 'motif-dialog-row';

        if (showSymbol) {
            const symbol = document.createElement('div');
            symbol.className = 'motif-dialog-symbol';
            symbol.setAttribute('aria-hidden', 'true');
            // Exclamation-in-triangle, drawn as crisp SVG (no AA on the strokes beyond
            // the era's feel). Yellow face / black outline — the classic warning glyph.
            symbol.innerHTML =
                "<svg viewBox='0 0 48 48' width='44' height='44'>" +
                "<path d='M24 4 L46 44 L2 44 Z' fill='#f4d000' stroke='#000' stroke-width='2' stroke-linejoin='miter'/>" +
                "<rect x='21' y='17' width='6' height='15' fill='#000'/>" +
                "<rect x='21' y='35' width='6' height='5' fill='#000'/>" +
                "</svg>";
            row.appendChild(symbol);
        }

        // Body slot: a string becomes the red message text; a Node is hosted verbatim
        // (the seam for a future minigame widget).
        const body = document.createElement('div');
        body.className = 'motif-dialog-body';
        if (typeof content === 'string') body.textContent = content;
        else body.appendChild(content);
        row.appendChild(body);
        dialog.appendChild(row);

        // Separator + action area (the OK button), as Motif lays out its message box.
        const sep = document.createElement('div');
        sep.className = 'motif-dialog-sep';
        dialog.appendChild(sep);

        const actions = document.createElement('div');
        actions.className = 'motif-dialog-actions';
        const okBtn = document.createElement('button');
        okBtn.type = 'button';
        okBtn.className = 'motif-dialog-ok';
        okBtn.textContent = buttonLabel;
        actions.appendChild(okBtn);
        dialog.appendChild(actions);

        overlay.appendChild(dialog);
        document.body.appendChild(overlay);

        // ── Dismissal (single path: OK button == Enter == Esc) ─────────────────
        let closed = false;
        const close = () => {
            if (closed) return;
            closed = true;
            document.removeEventListener('keydown', onKey, true);
            overlay.remove();
            activeRoot = null;
            activeClose = null;
            // Restore focus to wherever it was (e.g. the Update button) so keyboard
            // users land back where they started.
            try { prevFocus?.focus?.(); } catch (_) { /* element may be gone */ }
            resolve();
        };
        activeRoot = overlay;
        activeClose = close;

        okBtn.addEventListener('click', close);

        // Keyboard: Enter and Esc both fire OK; Tab is trapped on the lone OK button
        // (the default-button focus stays put — there's nowhere else to go), matching a
        // single-button modal.
        const onKey = (e: KeyboardEvent) => {
            if (e.key === 'Enter' || e.key === 'Escape') {
                e.preventDefault();
                e.stopPropagation();
                close();
            } else if (e.key === 'Tab') {
                // Keep focus on OK — full focus trap for a one-control dialog.
                e.preventDefault();
                okBtn.focus();
            }
        };
        document.addEventListener('keydown', onKey, true);

        // Click on the dark scrim does NOT dismiss — a full-application-modal warning
        // dialog only closes via its button (faithful to the original).

        // Focus the OK button on open so it shows the default-button focus rectangle.
        // rAF so layout has settled before we focus/scroll.
        requestAnimationFrame(() => okBtn.focus());
    });
}

/** Programmatically dismiss the open dialog, if any (e.g. on a screen change). */
export function closeMotifDialog(): void {
    activeClose?.();
}
