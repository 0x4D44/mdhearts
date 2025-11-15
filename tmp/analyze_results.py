import csv

seats = ['west', 'east', 'south', 'north']
results = {}

for seat in seats:
    with open(f'eval_{seat}_search.csv') as f:
        search_data = list(csv.reader(f))[1:]
        search_pens = [int(row[3].strip()) for row in search_data]

    with open(f'eval_{seat}_hard.csv') as f:
        hard_data = list(csv.reader(f))[1:]
        hard_pens = [int(row[3].strip()) for row in hard_data]

    search_avg = sum(search_pens) / len(search_pens)
    hard_avg = sum(hard_pens) / len(hard_pens)

    diffs = [i for i in range(len(search_pens)) if search_pens[i] != hard_pens[i]]

    results[seat] = {
        'search_avg': search_avg,
        'hard_avg': hard_avg,
        'delta': hard_avg - search_avg,
        'agreements': len(search_pens) - len(diffs),
        'disagreements': len(diffs),
        'pct_agree': (len(search_pens) - len(diffs)) / len(search_pens) * 100
    }

print('\n=== Search vs Hard Performance Analysis (100 seeds per seat) ===\n')
print('Seat     | Search Avg | Hard Avg | Delta  | Agreements | Disagreements | % Agree')
print('---------|------------|----------|--------|------------|---------------|--------')
for seat in seats:
    r = results[seat]
    print(f'{seat:8} | {r["search_avg"]:10.2f} | {r["hard_avg"]:8.2f} | {r["delta"]:+6.2f} | {r["agreements"]:10} | {r["disagreements"]:13} | {r["pct_agree"]:6.1f}%')

overall_search = sum(r['search_avg'] for r in results.values()) / len(results)
overall_hard = sum(r['hard_avg'] for r in results.values()) / len(results)
overall_delta = overall_hard - overall_search
overall_agree = sum(r['agreements'] for r in results.values()) / (len(results) * 100) * 100

print('---------|------------|----------|--------|------------|---------------|--------')
print(f'Overall  | {overall_search:10.2f} | {overall_hard:8.2f} | {overall_delta:+6.2f} | {"":10} | {"":13} | {overall_agree:6.1f}%')
