@echo off
REM Hearts RL Training Pipeline
REM
REM Quick start script for running the full RL training pipeline

echo ========================================
echo Hearts RL Training Pipeline
echo ========================================
echo.

REM Default parameters
set COLLECTION_GAMES=1000
set TRAINING_ITERS=100
set EVAL_GAMES=100

REM Parse arguments
:parse_args
if "%1"=="" goto run_pipeline
if "%1"=="--quick" (
    set COLLECTION_GAMES=100
    set TRAINING_ITERS=10
    set EVAL_GAMES=50
    shift
    goto parse_args
)
if "%1"=="--help" goto show_help
shift
goto parse_args

:run_pipeline
echo Configuration:
echo   Collection games: %COLLECTION_GAMES%
echo   Training iterations: %TRAINING_ITERS%
echo   Evaluation games: %EVAL_GAMES%
echo.

cd python

python -m hearts_rl.orchestrator ^
    --collection-games %COLLECTION_GAMES% ^
    --training-iterations %TRAINING_ITERS% ^
    --eval-games %EVAL_GAMES% ^
    --reward-mode shaped ^
    --baseline normal

cd ..

echo.
echo ========================================
echo Pipeline complete!
echo ========================================
goto end

:show_help
echo Usage: train_pipeline.bat [--quick] [--help]
echo.
echo Options:
echo   --quick    Run quick test (100 games, 10 iters)
echo   --help     Show this help message
echo.
echo Default: 1000 collection games, 100 training iterations, 100 eval games
goto end

:end
