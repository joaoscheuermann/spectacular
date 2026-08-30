# Case Data

The six JSON files are authored cases for this diagnostic. The eight focused
skills created with the lab are stored beside the vendored catalog in
`skills/`.

The other 29 skill files were materialized on 2026-08-30 from SkillsBench v1.1
commit `b63b7b2850226b6aa4fb5929a8c1ac7bc4d9a6af`. Their source files were under
`tasks/<task>/environment/skills/<skill>/SKILL.md` in
`benchflow-ai/skillsbench`. Frontmatter names and descriptions were converted
to the local Markdown heading and introductory paragraph; the skill bodies
were preserved. Runtime execution reads only these local files and performs no
network download.
