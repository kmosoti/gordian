"""R9: R8's reference reader (the prompt's stated rules, windowed) as a baseline on R9's questions.

Not the R9 reader. Kept so that the report can show what the stated rules alone get on the same contexts.
"""

import r8_rules


def read(services, focus, context):
    kind, site = r8_rules.read(services, focus, context)
    return kind, site, {}
