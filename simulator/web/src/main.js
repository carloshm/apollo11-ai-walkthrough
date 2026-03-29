/**
 * Main entry point for the AGC Simulator web interface.
 *
 * Initializes the WASM module, sets up UI event handlers,
 * and manages the simulation run loop.
 */

import { DskyInterface } from './dsky.js';
import { Debugger } from './debugger.js';
import { DEMO_PROGRAMS, getDemoPC } from './demos.js';

// Console logger
class Logger {
    constructor() {
        this.output = document.getElementById('console-output');
    }

    log(msg, cls = '') {
        const line = document.createElement('div');
        if (cls) line.className = cls;
        line.textContent = `[${this.timestamp()}] ${msg}`;
        this.output.appendChild(line);
        this.output.scrollTop = this.output.scrollHeight;
        // Keep only last 200 lines
        while (this.output.children.length > 200) {
            this.output.removeChild(this.output.firstChild);
        }
    }

    info(msg) { this.log(msg, 'log-info'); }
    warn(msg) { this.log(msg, 'log-warn'); }
    error(msg) { this.log(msg, 'log-error'); }

    timestamp() {
        const now = new Date();
        return now.toISOString().substring(11, 23);
    }
}

// Global state
let simulator = null;
let dsky = null;
let dbg = null;
let logger = null;
let running = false;
let animFrameId = null;

/**
 * Initialize the WASM module and set up the application.
 */
async function init() {
    logger = new Logger();
    logger.info('Apollo Guidance Computer Simulator starting...');

    try {
        // Import the WASM module
        const wasm = await import('../pkg/agc_wasm.js');
        await wasm.default();

        // Create simulator instance
        simulator = new wasm.AgcSimulator();
        logger.info('AGC simulator initialized (WASM)');
        logger.info('15-bit 1\'s complement · 2048 words RAM · 36864 words ROM');

        // Initialize UI modules
        dsky = new DskyInterface(simulator, logger);
        dbg = new Debugger(simulator, logger);

        setupControls();
        setupProgramLoader();

        // Initial display update
        dsky.blank();
        dbg.updateState();

        logger.info('Ready. Load a program to begin.');
    } catch (err) {
        logger.error(`Failed to initialize WASM: ${err.message}`);
        logger.warn('Make sure to build the WASM package first:');
        logger.warn('  cd simulator/agc-wasm && wasm-pack build --target web --out-dir ../web/pkg');
    }
}

/**
 * Set up execution control buttons.
 */
function setupControls() {
    document.getElementById('btn-step').addEventListener('click', () => {
        if (!simulator) return;
        if (!simulator.is_running()) {
            simulator.start();
        }
        simulator.step();
        dbg.updateState();
        dsky.updateDisplay();
    });

    document.getElementById('btn-run').addEventListener('click', startRunning);
    document.getElementById('btn-stop').addEventListener('click', stopRunning);

    document.getElementById('btn-reset').addEventListener('click', () => {
        stopRunning();
        simulator.reset();
        dsky.blank();
        dbg.updateState();
        logger.info('AGC reset');
    });

    document.getElementById('btn-gojam').addEventListener('click', () => {
        if (!simulator) return;
        simulator.gojam();
        simulator.start();
        dbg.updateState();
        dsky.updateDisplay();
        logger.warn('GOJAM — Hardware reset triggered');
    });

    // Speed slider
    const slider = document.getElementById('speed-slider');
    slider.addEventListener('input', () => {
        dbg.updateSpeedLabel(parseInt(slider.value));
    });
    dbg.updateSpeedLabel(parseInt(slider.value));
}

/**
 * Start continuous execution.
 */
function startRunning() {
    if (!simulator || running) return;
    simulator.start();
    running = true;
    logger.info('Execution started');
    runLoop();
}

/**
 * Stop continuous execution.
 */
function stopRunning() {
    running = false;
    if (animFrameId) {
        cancelAnimationFrame(animFrameId);
        animFrameId = null;
    }
    if (simulator) {
        simulator.stop();
    }
    dbg.updateState();
    dsky.updateDisplay();
    logger.info('Execution stopped');
}

/**
 * Main run loop — executes a batch of instructions per animation frame.
 */
function runLoop() {
    if (!running || !simulator) return;

    const slider = document.getElementById('speed-slider');
    const mctsPerFrame = dbg.sliderToMcts(parseInt(slider.value));

    simulator.run_mcts(mctsPerFrame);

    // Update display at 60fps rate
    dbg.updateState();
    dsky.updateDisplay();

    // Check if still running
    if (!simulator.is_running()) {
        stopRunning();
        logger.info('CPU halted');
        return;
    }

    animFrameId = requestAnimationFrame(runLoop);
}

/**
 * Set up program loader (demo programs and custom input).
 */
function setupProgramLoader() {
    // Tab switching
    document.querySelectorAll('.loader-tab').forEach(tab => {
        tab.addEventListener('click', () => {
            document.querySelectorAll('.loader-tab').forEach(t => t.classList.remove('active'));
            tab.classList.add('active');

            document.querySelectorAll('.loader-content').forEach(c => c.classList.add('hidden'));
            const target = tab.dataset.tab;
            document.getElementById(`loader-${target}`).classList.remove('hidden');
        });
    });

    // Demo program buttons
    document.querySelectorAll('.demo-btn').forEach(btn => {
        btn.addEventListener('click', () => {
            const demoName = btn.dataset.demo;
            loadDemo(demoName);
        });
    });

    // Custom code loader
    document.getElementById('btn-load').addEventListener('click', loadCustom);
}

/**
 * Load a built-in demo program.
 */
function loadDemo(name) {
    const demo = DEMO_PROGRAMS[name];
    if (!demo) {
        logger.error(`Unknown demo: ${name}`);
        return;
    }

    stopRunning();
    simulator.reset();

    // Load program words into the specified bank
    const words = new Uint16Array(demo.words);
    simulator.load_program(demo.bank, words);

    // Set PC to program start
    const pc = getDemoPC(demo);
    simulator.set_pc(pc);
    simulator.start();

    // Navigate memory inspector to program start
    dbg.navigateMemory(pc.toString(8));

    dsky.blank();
    dbg.updateState();

    logger.info(`Loaded demo: ${demo.name}`);
    logger.info(demo.description);
    logger.info(`Program at bank ${demo.bank} (address ${pc.toString(8)}), ${demo.words.length} words`);
}

/**
 * Load custom machine code from the text area.
 */
function loadCustom() {
    const codeText = document.getElementById('custom-code').value.trim();
    if (!codeText) {
        logger.error('No code entered');
        return;
    }

    const bank = parseInt(document.getElementById('load-bank').value);
    const addrStr = document.getElementById('load-addr').value;
    const startAddr = parseInt(addrStr, 8);

    if (isNaN(bank) || bank < 0 || bank > 35) {
        logger.error('Invalid bank number (0–35)');
        return;
    }

    // Parse octal words
    const lines = codeText.split('\n');
    const words = [];
    for (const line of lines) {
        const trimmed = line.trim();
        if (!trimmed || trimmed.startsWith('#') || trimmed.startsWith('//')) continue;
        const val = parseInt(trimmed, 8);
        if (isNaN(val)) {
            logger.error(`Invalid octal value: "${trimmed}"`);
            return;
        }
        words.push(val & 0x7FFF);
    }

    if (words.length === 0) {
        logger.error('No valid words parsed');
        return;
    }

    stopRunning();
    simulator.reset();

    const data = new Uint16Array(words);
    simulator.load_program(bank, data);
    simulator.set_pc(startAddr);
    simulator.start();

    dbg.navigateMemory(startAddr.toString(8));
    dsky.blank();
    dbg.updateState();

    logger.info(`Loaded ${words.length} words at bank ${bank} (address ${startAddr.toString(8)})`);
}

// Start the application
init();
