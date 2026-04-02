# Research Summarization Prompt Template
# Use: When summarizing a paper, article, or body of literature for research copilot sessions.
# Layer: App/Domain (Layer 2) — procedural memory for the research_copilot app

---

## Summarize: {document_title}

**Task:** Produce a structured summary suitable for a research registry entry.

**Source:** {source_url_or_doi}

**Output format:**

### Citation
{Authors}. ({Year}). *{Title}*. *{Venue/Journal}*, {Volume/Issue/Pages}.
DOI: {DOI} | Verified via: {verification_source}

### Core Claim
One sentence. What does this work claim or demonstrate? Use evidence classification:
- If peer-reviewed and replicated: state directly
- If single study: "X et al. found that..."
- If theoretical/modeling: "X et al. propose that..."

### Methodology
2-3 sentences. How did they demonstrate the claim? What were the sample sizes, methods, or datasets?

### Key Findings
- Finding 1 (evidence level: {verified|established|inferred|theoretical})
- Finding 2
- Finding 3 (max 5)

### Limitations
What did the authors acknowledge as limitations? What methodological concerns apply?

### Relevance to {project_context}
1-2 sentences. Why does this matter for the current project/domain?

### Contradicts or Confirms
Does this work confirm or contradict any existing knowledge in the research registry?
If conflict: flag explicitly.

---

## Anti-Hallucination Gate

Before outputting the summary, verify:
- [ ] DOI resolves via `https://api.crossref.org/works/{DOI}` → status 200
- [ ] Author names match Crossref metadata
- [ ] Findings direction is correct (positive result vs. null result vs. negative result)
- [ ] Venue name matches the publication record
- [ ] Year matches the publication record

If any check fails: mark the field as `UNVERIFIED` and note which check failed.
Do not present unverified claims as supporting evidence.
