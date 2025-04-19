// RuState Wasm モジュールをインポート
import {
    Machine,
    State,
    Transition,
    init,
} from './../pkg/rustate.js';

let currentTrack = 0;

// 交通信号機のUI更新関数（Rust側から呼び出される）
window.updateTrafficLightUI = (state) => {
    console.log('Traffic light state updated:', state);
    
    // すべての信号をリセット
    document.querySelectorAll('.light').forEach(light => {
        light.classList.remove('active');
    });
    
    // 現在の状態に対応するライトをアクティブにする
    const activeLight = document.getElementById(`${state}-light`);
    if (activeLight) {
        activeLight.classList.add('active');
    }
    
    // 状態表示テキストを更新
    const stateElement = document.getElementById('traffic-state');
    if (stateElement) {
        stateElement.textContent = state;
    }
};

// 音楽プレーヤーのUI更新関数（Rust側から呼び出される）
window.updateMusicPlayerUI = (statesJson) => {
    try {
        const states = JSON.parse(statesJson);
        console.log('Music player states updated:', states);
        
        // 状態表示の更新
        const statusElement = document.getElementById('player-status');
        if (statusElement) {
            statusElement.textContent = `状態: ${states.join(', ')}`;
        }
        
        // UIの更新 - 電源状態に基づくボタンの有効化/無効化
        const isPowerOff = states.includes('powerOff');
        const isPlaying = states.includes('playing');
        const isPaused = states.includes('paused');
        const isNormal = states.includes('normal');
        
        // 電源に応じてボタンを有効/無効化
        document.querySelectorAll('#play-btn, #pause-btn, #stop-btn, #prev-btn, #next-btn, #speed-up-btn, #speed-normal-btn')
            .forEach(btn => {
                btn.disabled = isPowerOff;
            });
            
        // 特定の状態に基づいてボタンの有効/無効化
        document.getElementById('play-btn').disabled = isPowerOff || isPlaying;
        document.getElementById('pause-btn').disabled = isPowerOff || isPaused || !isPlaying;
        document.getElementById('stop-btn').disabled = isPowerOff || !isPlaying;
        document.getElementById('speed-up-btn').disabled = isPowerOff || !isPlaying || !isNormal;
        document.getElementById('speed-normal-btn').disabled = isPowerOff || !isPlaying || isNormal;
    } catch (error) {
        console.error('Failed to parse states JSON:', error);
    }
};

// Wasm初期化とイベントリスナーの設定
async function initWasm() {
    try {
        // Wasmモジュールを初期化
        await init();
        console.log('Wasm module loaded');
        
        // 交通信号機の初期化
        await init_traffic_light();
        
        // 音楽プレーヤーの初期化
        await init_music_player();
        
        // イベントリスナーのセットアップ
        setupEventListeners();
    } catch (error) {
        console.error('Failed to initialize Wasm:', error);
        document.body.innerHTML = `<h1>エラー</h1><p>Wasmの初期化に失敗しました: ${error.message}</p>`;
    }
}

function setupEventListeners() {
    // 交通信号機のイベントリスナー
    document.getElementById('traffic-timer-btn').addEventListener('click', () => {
        try {
            send_traffic_light_event('TIMER');
        } catch (error) {
            console.error('Failed to send traffic light event:', error);
        }
    });
    
    // 音楽プレーヤーのイベントリスナー
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
    // トラック表示の更新（コンテキストからの値はRustで処理されるが、UIの即時更新のため）
    const trackElement = document.getElementById('player-track');
    if (trackElement) {
        // NEXTイベントではインクリメント、PREVイベントではデクリメント（最小値は0）
        const action = event === 'NEXT' ? 1 : -1;
        currentTrack = Math.max(0, currentTrack + action);
        trackElement.textContent = `トラック: ${currentTrack}`;
    }
}

// Wasm初期化を実行
initWasm().catch(console.error); 