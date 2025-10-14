@echo off
REM Gen4 Training Launch Script
REM Research-informed hybrid BC+RL approach
REM Based on: docs/GEN4_STRATEGY.md

echo.
echo ============================================================
echo Gen4 Hearts RL Training - Hybrid BC+RL Approach
echo ============================================================
echo.
echo This script will:
echo 1. Collect diverse opponent training data (40k games, ~2 hours)
echo 2. Launch Gen4 training (150 iterations, ~18 hours)
echo.
echo Total estimated time: ~20 hours
echo.
pause

REM Step 1: Collect diverse opponent data
echo.
echo ============================================================
echo Step 1/5: Collecting data vs Easy opponent (10k games)
echo ============================================================
echo.
cargo build --release
if %ERRORLEVEL% NEQ 0 (
    echo Error: Build failed!
    pause
    exit /b 1
)

.\target\release\mdhearts.exe eval 10000 --ai easy --ai-test embedded --weights ai_training\bc\bc_hard_20ep_10k.json --collect-rl gen4_vs_easy.jsonl --reward-mode shaped
if %ERRORLEVEL% NEQ 0 (
    echo Error: Data collection vs Easy failed!
    pause
    exit /b 1
)

echo.
echo ============================================================
echo Step 2/5: Collecting data vs Normal opponent (10k games)
echo ============================================================
echo.
.\target\release\mdhearts.exe eval 10000 --ai normal --ai-test embedded --weights ai_training\bc\bc_hard_20ep_10k.json --collect-rl gen4_vs_normal.jsonl --reward-mode shaped
if %ERRORLEVEL% NEQ 0 (
    echo Error: Data collection vs Normal failed!
    pause
    exit /b 1
)

echo.
echo ============================================================
echo Step 3/5: Collecting data vs Hard opponent (10k games)
echo ============================================================
echo.
.\target\release\mdhearts.exe eval 10000 --ai hard --ai-test embedded --weights ai_training\bc\bc_hard_20ep_10k.json --collect-rl gen4_vs_hard.jsonl --reward-mode shaped
if %ERRORLEVEL% NEQ 0 (
    echo Error: Data collection vs Hard failed!
    pause
    exit /b 1
)

echo.
echo ============================================================
echo Step 4/5: Collecting self-play data (10k games)
echo ============================================================
echo.
.\target\release\mdhearts.exe eval 10000 --self-play --weights ai_training\bc\bc_hard_20ep_10k.json --collect-rl gen4_selfplay.jsonl --reward-mode shaped
if %ERRORLEVEL% NEQ 0 (
    echo Error: Self-play data collection failed!
    pause
    exit /b 1
)

echo.
echo ============================================================
echo Merging all training data...
echo ============================================================
echo.
REM Verify all files exist before merging
if not exist gen4_vs_easy.jsonl (
    echo Error: gen4_vs_easy.jsonl not found!
    pause
    exit /b 1
)
if not exist gen4_vs_normal.jsonl (
    echo Error: gen4_vs_normal.jsonl not found!
    pause
    exit /b 1
)
if not exist gen4_vs_hard.jsonl (
    echo Error: gen4_vs_hard.jsonl not found!
    pause
    exit /b 1
)
if not exist gen4_selfplay.jsonl (
    echo Error: gen4_selfplay.jsonl not found!
    pause
    exit /b 1
)

copy /b gen4_vs_easy.jsonl+gen4_vs_normal.jsonl+gen4_vs_hard.jsonl+gen4_selfplay.jsonl gen4_mixed.jsonl
if %ERRORLEVEL% NEQ 0 (
    echo Error: Failed to merge data files!
    pause
    exit /b 1
)

echo.
echo ============================================================
echo Data collection complete!
echo ============================================================
echo Files created:
dir /b gen4*.jsonl
echo.
echo ============================================================
echo Step 5/5: Launching Gen4 training
echo ============================================================
echo.
echo Configuration:
echo - Algorithm: PPO with BC regularization
echo - Learning rate: 1e-4 (conservative)
echo - Clip epsilon: 0.1 (tight clipping)
echo - BC lambda: 0.1 (regularization strength)
echo - Iterations: 150
echo - Checkpoint interval: 5
echo - Expected duration: ~18 hours
echo.
echo Training will run in the background.
echo Monitor progress: python/gen4_logs/
echo Checkpoints: gen4_checkpoints/
echo.
pause

cd python
python -m hearts_rl.train ^
  --data ..\gen4_mixed.jsonl ^
  --resume ..\ai_training\bc\bc_hard_20ep_10k.json ^
  --output ..\gen4_weights.json ^
  --iterations 150 ^
  --lr 1e-4 ^
  --clip-epsilon 0.1 ^
  --bc-lambda 0.1 ^
  --bc-reference ..\ai_training\bc\bc_hard_20ep_10k.json ^
  --save-interval 5 ^
  --checkpoint-dir ..\gen4_checkpoints ^
  --log-dir ..\gen4_logs

if %ERRORLEVEL% NEQ 0 (
    cd ..
    echo Error: Training failed!
    pause
    exit /b 1
)

cd ..

echo.
echo ============================================================
echo Gen4 Training Complete!
echo ============================================================
echo.
echo Files created:
echo - gen4_weights.json (final model)
echo - gen4_checkpoints/ (30 checkpoints: 5, 10, 15...150)
echo - gen4_logs/ (TensorBoard logs)
echo.
echo Next steps:
echo 1. Evaluate checkpoints: python tools\eval_checkpoint.py gen4_checkpoints\checkpoint_50.pt --games 200
echo 2. Compare to BC Hard baseline
echo 3. Analyze results
echo.
pause
