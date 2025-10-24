Deterministic Flip Seeds (as of 2025-10-22)

Locked in tests
- 2031/East → Normal=9♠, Hard=A♦
- 1145/North → Normal=J♠, Hard=A♦
- 1080/South → Normal=J♠, Hard=7♠
- 2044/East → Normal=9♠, Hard=2♠

Disagreement CSVs (deterministic, only disagreements)
- designs/tuning/compare_west_1000_50.csv
- designs/tuning/compare_east_2000_50.csv
- designs/tuning/compare_north_1100_50.csv
- designs/tuning/compare_south_1080_50.csv

Reproduce (examples)
- mdhearts --compare-once 2031 east --hard-deterministic --hard-steps 80
- mdhearts --compare-batch west 1000 50 --only-disagree --hard-deterministic --hard-steps 80 --out designs/tuning/compare_west_1000_50.csv
