/**
 * DSKY (Display & Keyboard) interface module.
 *
 * Handles the visual DSKY display and keyboard input,
 * translating between the AGC simulator and the web UI.
 */

// DSKY key codes (as sent to AGC via KEYRUPT1)
const KEY_CODES = {
    '0': 0o20,
    '1': 0o01,
    '2': 0o02,
    '3': 0o03,
    '4': 0o04,
    '5': 0o05,
    '6': 0o06,
    '7': 0o07,
    '8': 0o10,
    '9': 0o11,
    'verb': 0o21,
    'noun': 0o31,
    'plus': 0o32,
    'minus': 0o33,
    'clr': 0o36,
    'pro': 0o34,
    'key-rel': 0o31,
    'enter': 0o34,
    'reset': 0o22,
};

export class DskyInterface {
    constructor(simulator, logger) {
        this.simulator = simulator;
        this.logger = logger;

        // Display element references
        this.progDisplay = document.getElementById('dsky-prog');
        this.verbDisplay = document.getElementById('dsky-verb');
        this.nounDisplay = document.getElementById('dsky-noun');
        this.r1Display = document.getElementById('dsky-r1');
        this.r2Display = document.getElementById('dsky-r2');
        this.r3Display = document.getElementById('dsky-r3');

        this.setupKeyboard();
    }

    setupKeyboard() {
        document.querySelectorAll('.dsky-key').forEach(btn => {
            btn.addEventListener('click', () => {
                const key = btn.dataset.key;
                this.handleKeyPress(key);
            });
        });

        // Keyboard shortcuts
        document.addEventListener('keydown', (e) => {
            if (e.target.tagName === 'INPUT' || e.target.tagName === 'TEXTAREA') return;

            const keyMap = {
                '0': '0', '1': '1', '2': '2', '3': '3', '4': '4',
                '5': '5', '6': '6', '7': '7', '8': '8', '9': '9',
                'v': 'verb', 'n': 'noun', '+': 'plus', '-': 'minus',
                'Enter': 'enter', 'Escape': 'clr', 'p': 'pro',
                'r': 'reset',
            };

            const dskyKey = keyMap[e.key];
            if (dskyKey) {
                e.preventDefault();
                this.handleKeyPress(dskyKey);
                // Visual feedback
                const btn = document.querySelector(`[data-key="${dskyKey}"]`);
                if (btn) {
                    btn.classList.add('active');
                    setTimeout(() => btn.classList.remove('active'), 150);
                }
            }
        });
    }

    handleKeyPress(key) {
        const code = KEY_CODES[key];
        if (code !== undefined && this.simulator) {
            this.simulator.dsky_key_press(code);
            this.logger.info(`DSKY key: ${key.toUpperCase()} (code: ${code.toString(8).padStart(2, '0')})`);
        }
    }

    /**
     * Update the DSKY display from current AGC state.
     * In a real AGC, the DSPTAB buffer drives the display.
     * For this simulator, we read from erasable memory.
     */
    updateDisplay() {
        if (!this.simulator) return;

        // Read display state from simulator
        // DSPTAB is at erasable addresses around 0o11D (decimal 11) area
        // For the demo, show register values in the DSKY readouts
        const state = this.simulator.get_state();
        if (!state) return;

        // Show PC in PROG field
        this.progDisplay.textContent = this.formatOctal2(state.z >> 8);

        // Show A register value in R1
        this.r1Display.textContent = this.formatSignedOctal(state.a);

        // Show L register value in R2
        this.r2Display.textContent = this.formatSignedOctal(state.l);

        // Show a temp memory value in R3
        const temp1 = this.simulator.read_memory(0o100);
        this.r3Display.textContent = this.formatSignedOctal(temp1);

        // Update status lights
        this.updateLight('light-comp-acty', state.running);
        this.updateLight('light-restart', false);
        this.updateLight('light-stby', !state.running);
    }

    formatOctal2(val) {
        return (val & 0o77).toString(8).padStart(2, '0');
    }

    formatSignedOctal(val) {
        const v = val & 0x7FFF;
        const isNeg = (v & 0x4000) !== 0;
        const magnitude = isNeg ? (~v & 0x3FFF) : v;
        const sign = isNeg ? '-' : '+';
        return sign + magnitude.toString(10).padStart(5, '0');
    }

    updateLight(id, active) {
        const el = document.getElementById(id);
        if (el) {
            el.classList.toggle('active', active);
        }
    }

    /**
     * Blank all displays.
     */
    blank() {
        const blanked = '     ';
        this.progDisplay.textContent = '--';
        this.verbDisplay.textContent = '--';
        this.nounDisplay.textContent = '--';
        this.r1Display.textContent = blanked;
        this.r2Display.textContent = blanked;
        this.r3Display.textContent = blanked;

        // Turn off all status lights
        document.querySelectorAll('.status-light').forEach(el => {
            el.classList.remove('active');
        });
    }
}
