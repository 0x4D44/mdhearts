#!/bin/bash
# Monitor Gen3 training and evaluate checkpoints automatically

GEN3_CHECKPOINT_DIR="gen3_checkpoints"
GEN3_LOGS_DIR="gen3_logs"
EVAL_RESULTS="gen3_checkpoint_evals.txt"

echo "Gen3 Training Monitor" > $EVAL_RESULTS
echo "=====================" >> $EVAL_RESULTS
echo "" >> $EVAL_RESULTS

# Function to export checkpoint to JSON and evaluate
evaluate_checkpoint() {
    local checkpoint=$1
    local iteration=$2

    echo "Evaluating checkpoint iteration $iteration..." | tee -a $EVAL_RESULTS

    # Export checkpoint to JSON
    cd python
    python export_checkpoint.py --checkpoint "../$checkpoint" --output "../gen3_iter${iteration}.json"
    cd ..

    # Quick evaluation: 100 games vs Hard
    echo "  Running 100 games vs Hard..." | tee -a $EVAL_RESULTS
    ./target/release/mdhearts.exe eval 100 \
        --ai hard \
        --ai-test embedded \
        --weights "gen3_iter${iteration}.json" > "eval_iter${iteration}_vs_hard.txt"

    # Extract key metrics
    WIN_RATE=$(grep "Improvement:" "eval_iter${iteration}_vs_hard.txt" | awk '{print $2}')
    P_VALUE=$(grep "P-value:" "eval_iter${iteration}_vs_hard.txt" | awk '{print $2}')

    echo "  Iteration $iteration: Win rate change: $WIN_RATE, p-value: $P_VALUE" | tee -a $EVAL_RESULTS
    echo "" >> $EVAL_RESULTS
}

# Monitor for new checkpoints
echo "Monitoring for Gen3 checkpoints..."
echo "Checkpoints will appear at iterations 50 and 100"
echo ""

LAST_CHECKPOINT=""

while true; do
    # Check for checkpoint_50.pt
    if [ -f "$GEN3_CHECKPOINT_DIR/checkpoint_50.pt" ] && [ "$LAST_CHECKPOINT" != "50" ]; then
        echo "Found checkpoint 50!"
        evaluate_checkpoint "$GEN3_CHECKPOINT_DIR/checkpoint_50.pt" 50
        LAST_CHECKPOINT="50"
    fi

    # Check for checkpoint_100.pt
    if [ -f "$GEN3_CHECKPOINT_DIR/checkpoint_100.pt" ] && [ "$LAST_CHECKPOINT" != "100" ]; then
        echo "Found checkpoint 100!"
        evaluate_checkpoint "$GEN3_CHECKPOINT_DIR/checkpoint_100.pt" 100
        LAST_CHECKPOINT="100"
        echo "Training complete! Final checkpoint evaluated."
        break
    fi

    # Check if final weights exist (training done)
    if [ -f "gen3_weights.json" ]; then
        echo "Training complete! Final weights found."
        echo "Running final evaluation (1000 games)..."

        # More thorough final evaluation
        ./target/release/mdhearts.exe eval 1000 \
            --ai hard \
            --ai-test embedded \
            --weights "gen3_weights.json" > "eval_gen3_final_vs_hard.txt"

        WIN_RATE=$(grep "Improvement:" "eval_gen3_final_vs_hard.txt" | awk '{print $2}')
        P_VALUE=$(grep "P-value:" "eval_gen3_final_vs_hard.txt" | awk '{print $2}')

        echo "Final Gen3: Win rate change: $WIN_RATE, p-value: $P_VALUE" | tee -a $EVAL_RESULTS
        break
    fi

    # Wait before checking again
    sleep 60
done

echo ""
echo "Monitoring complete. Results saved to $EVAL_RESULTS"
cat $EVAL_RESULTS
