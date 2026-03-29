/**
 * CPU debugger/state display module.
 *
 * Updates the register display, flags, stats, and memory inspector.
 */

export class Debugger {
    constructor(simulator, logger) {
        this.simulator = simulator;
        this.logger = logger;
        this.prevState = null;
        this.memStartAddr = 0;

        this.setupMemoryInspector();
    }

    setupMemoryInspector() {
        const goBtn = document.getElementById('btn-mem-go');
        const addrInput = document.getElementById('mem-addr');

        goBtn.addEventListener('click', () => {
            this.navigateMemory(addrInput.value);
        });

        addrInput.addEventListener('keydown', (e) => {
            if (e.key === 'Enter') {
                this.navigateMemory(addrInput.value);
            }
        });

        // Initial memory display
        this.navigateMemory('0000');
    }

    navigateMemory(addrStr) {
        const addr = parseInt(addrStr, 8);
        if (isNaN(addr) || addr < 0 || addr > 0o7777) {
            this.logger.error(`Invalid octal address: ${addrStr}`);
            return;
        }
        this.memStartAddr = addr;
        this.updateMemoryView();
    }

    /**
     * Update all CPU state displays.
     */
    updateState() {
        if (!this.simulator) return;

        const state = this.simulator.get_state();
        if (!state) return;

        // Update registers
        this.updateReg('reg-a', state.a, this.prevState?.a);
        this.updateReg('reg-l', state.l, this.prevState?.l);
        this.updateReg('reg-q', state.q, this.prevState?.q);
        this.updateReg('reg-z', state.z, this.prevState?.z);
        this.updateReg('reg-eb', state.eb, this.prevState?.eb, 1);
        this.updateReg('reg-fb', state.fb, this.prevState?.fb);
        this.updateReg('reg-bb', state.bb, this.prevState?.bb);

        // Update flags
        this.updateFlag('flag-overflow', state.a_overflow);
        this.updateFlag('flag-extend', state.extend);
        this.updateFlag('flag-interrupts', state.interrupts_enabled);
        this.updateFlag('flag-isr', state.in_isr, true);
        this.updateFlag('flag-running', state.running);
        this.updateFlag('flag-engine', this.simulator.is_engine_on(), true);

        // Update stats
        document.getElementById('stat-instructions').textContent =
            state.instruction_count.toLocaleString();
        document.getElementById('stat-elapsed').textContent =
            state.elapsed_ms.toFixed(3) + ' ms';

        this.prevState = state;

        // Update memory if auto-refresh is on
        const autoRefresh = document.getElementById('mem-auto-refresh');
        if (autoRefresh && autoRefresh.checked) {
            this.updateMemoryView();
        }
    }

    updateReg(id, value, prevValue, digits = 5) {
        const el = document.getElementById(id);
        if (!el) return;
        el.textContent = value.toString(8).padStart(digits, '0');
        if (prevValue !== undefined && value !== prevValue) {
            el.classList.add('changed');
            setTimeout(() => el.classList.remove('changed'), 300);
        }
    }

    updateFlag(id, active, isWarn = false) {
        const el = document.getElementById(id);
        if (!el) return;
        el.classList.toggle('active', active && !isWarn);
        el.classList.toggle('warn', active && isWarn);
    }

    updateMemoryView() {
        const container = document.getElementById('memory-view');
        if (!container || !this.simulator) return;

        const rows = parseInt(document.getElementById('mem-rows')?.value || '16');
        const wordsPerRow = 8;
        const totalWords = rows * wordsPerRow;
        const pcAddr = this.simulator.reg_z();

        let html = '';
        for (let row = 0; row < rows; row++) {
            const baseAddr = this.memStartAddr + row * wordsPerRow;
            html += '<div class="mem-row">';
            html += `<span class="mem-addr">${baseAddr.toString(8).padStart(5, '0')}:</span>`;

            for (let col = 0; col < wordsPerRow; col++) {
                const addr = baseAddr + col;
                if (addr > 0o7777) {
                    html += '<span class="mem-word">-----</span>';
                } else {
                    const val = this.simulator.read_memory(addr);
                    const isPC = addr === pcAddr;
                    const cls = isPC ? 'mem-word highlight' : 'mem-word';
                    html += `<span class="${cls}" title="Addr: ${addr.toString(8)}">${val.toString(8).padStart(5, '0')}</span>`;
                }
            }
            html += '</div>';
        }

        container.innerHTML = html;
    }

    /**
     * Update the speed label based on slider value.
     */
    updateSpeedLabel(sliderValue) {
        const speedLabel = document.getElementById('speed-label');
        if (!speedLabel) return;

        // Map slider 1–100 to MCTs per frame
        const mcts = this.sliderToMcts(sliderValue);
        const instsPerSec = (mcts / 2) * 60; // Rough: 2 MCTs per instruction, 60 fps
        if (instsPerSec >= 1000) {
            speedLabel.textContent = `~${(instsPerSec / 1000).toFixed(0)}K inst/s`;
        } else {
            speedLabel.textContent = `~${instsPerSec.toFixed(0)} inst/s`;
        }
    }

    sliderToMcts(value) {
        // Exponential scale: 1 → 2 MCTs (single step speed), 100 → ~85,000 MCTs (~1 real-time ms)
        return Math.floor(2 * Math.pow(1.11, value));
    }
}
