// Import RuState Wasm module directly with correct path
import {
    Machine,
    State,
    Transition,
    init,
    init_traffic_light,
    init_music_player,
    send_traffic_light_event,
    send_music_player_event
} from './pkg/rustate.js';

let currentTrack = 0;

// Traffic light UI update function (called from Rust)
window.updateTrafficLightUI = (state) => {
    console.log('Traffic light state updated:', state);
    
    // Reset all signals
    document.querySelectorAll('.light').forEach(light => {
        light.classList.remove('active');
    });
    
    // Activate the light corresponding to the current state
    const activeLight = document.getElementById(`${state}-light`);
    if (activeLight) {
        activeLight.classList.add('active');
    }
    
    // Update state display text
    const stateElement = document.getElementById('traffic-state');
    if (stateElement) {
        stateElement.textContent = state;
    }
};

// Music player UI update function (called from Rust)
window.updateMusicPlayerUI = (statesJson) => {
    try {
        const states = JSON.parse(statesJson);
        console.log('Music player states updated:', states);
        
        // Update state display
        const statusElement = document.getElementById('player-status');
        if (statusElement) {
            statusElement.textContent = `Status: ${states.join(', ')}`;
        }
        
        // Update UI - enable/disable buttons based on power state
        const isPowerOff = states.includes('powerOff');
        const isPlaying = states.includes('playing');
        const isPaused = states.includes('paused');
        const isNormal = states.includes('normal');
        
        // Enable/disable buttons based on power state
        document.querySelectorAll('#play-btn, #pause-btn, #stop-btn, #prev-btn, #next-btn, #speed-up-btn, #speed-normal-btn')
            .forEach(btn => {
                btn.disabled = isPowerOff;
            });
            
        // Enable/disable buttons based on specific states
        document.getElementById('play-btn').disabled = isPowerOff || isPlaying;
        document.getElementById('pause-btn').disabled = isPowerOff || isPaused || !isPlaying;
        document.getElementById('stop-btn').disabled = isPowerOff || !isPlaying;
        document.getElementById('speed-up-btn').disabled = isPowerOff || !isPlaying || !isNormal;
        document.getElementById('speed-normal-btn').disabled = isPowerOff || !isPlaying || isNormal;
    } catch (error) {
        console.error('Failed to parse states JSON:', error);
    }
};

// Wasm initialization and event listener setup
async function initWasm() {
    try {
        console.log('Attempting to initialize Wasm module...');
        // Initialize Wasm module
        await init();
        console.log('Wasm module loaded successfully');
        
        // Check if initialization functions exist
        console.log('Checking for init_traffic_light function:', typeof init_traffic_light);
        console.log('Checking for init_music_player function:', typeof init_music_player);
        
        // Initialize traffic light - with catch block to handle errors
        try {
            console.log('Starting traffic light initialization...');
            await init_traffic_light();
            console.log('Traffic light initialized');
        } catch (error) {
            console.error('Error initializing traffic light:', error);
            document.getElementById('traffic-state').textContent = `Error: ${error.message}`;
        }
        
        // Initialize music player - with catch block to handle errors
        try {
            console.log('Starting music player initialization...');
            await init_music_player();
            console.log('Music player initialized');
        } catch (error) {
            console.error('Error initializing music player:', error);
            document.getElementById('player-status').textContent = `Error: ${error.message}`;
        }
        
        // Set up event listeners
        setupEventListeners();
        console.log('Event listeners set up');
    } catch (error) {
        console.error('Failed to initialize Wasm:', error);
        console.error('Error details:', {
            name: error.name,
            message: error.message,
            stack: error.stack
        });
        document.body.innerHTML = `<h1>Error</h1><p>Failed to initialize Wasm: ${error.message}</p><pre>${error.stack}</pre>`;
    }
}

function setupEventListeners() {
    // Traffic light event listeners
    document.getElementById('traffic-timer-btn').addEventListener('click', () => {
        try {
            console.log('Sending traffic light TIMER event');
            send_traffic_light_event('TIMER');
        } catch (error) {
            console.error('Failed to send traffic light event:', error);
        }
    });
    
    // Music player event listeners
    document.getElementById('power-btn').addEventListener('click', () => {
        try {
            send_music_player_event('POWER');
        } catch (error) {
            console.error('Failed to send music player event:', error);
        }
    });
    
    document.getElementById('play-btn').addEventListener('click', () => {
        try {
            send_music_player_event('PLAY');
        } catch (error) {
            console.error('Failed to send music player event:', error);
        }
    });
    
    document.getElementById('pause-btn').addEventListener('click', () => {
        try {
            send_music_player_event('PAUSE');
        } catch (error) {
            console.error('Failed to send music player event:', error);
        }
    });
    
    document.getElementById('stop-btn').addEventListener('click', () => {
        try {
            send_music_player_event('STOP');
        } catch (error) {
            console.error('Failed to send music player event:', error);
        }
    });
    
    document.getElementById('prev-btn').addEventListener('click', () => {
        try {
            send_music_player_event('PREV');
            updateTrackDisplay('PREV');
        } catch (error) {
            console.error('Failed to send music player event:', error);
        }
    });
    
    document.getElementById('next-btn').addEventListener('click', () => {
        try {
            send_music_player_event('NEXT');
            updateTrackDisplay('NEXT');
        } catch (error) {
            console.error('Failed to send music player event:', error);
        }
    });
    
    document.getElementById('speed-up-btn').addEventListener('click', () => {
        try {
            send_music_player_event('SPEED_UP');
        } catch (error) {
            console.error('Failed to send music player event:', error);
        }
    });
    
    document.getElementById('speed-normal-btn').addEventListener('click', () => {
        try {
            send_music_player_event('SPEED_NORMAL');
        } catch (error) {
            console.error('Failed to send music player event:', error);
        }
    });
}

function updateTrackDisplay(event) {
    // Update track display (values from context are processed in Rust, but this is for immediate UI update)
    const trackElement = document.getElementById('player-track');
    if (trackElement) {
        // Increment for NEXT event, decrement for PREV event (minimum value is 0)
        const action = event === 'NEXT' ? 1 : -1;
        currentTrack = Math.max(0, currentTrack + action);
        trackElement.textContent = `Track: ${currentTrack}`;
    }
}

// Run Wasm initialization
initWasm().catch(console.error);
