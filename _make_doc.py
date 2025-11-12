# Python script to write architecture doc
import codecs

doc_text = '''See script for content'''

with codecs.open(r'wrk_docs\2025.11.06 - Architecture - hearts-core crate.md', 'w', encoding='utf-8') as f:
    f.write('# Test')

print('Done')
