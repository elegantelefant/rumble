ABOUTME: record of ivory ui audit execution and structural updates.
ABOUTME: documents workflow decisions from feb 14 redesign.

# Front-end Audit Execution Notes

- Sidebar now features Elefant mark at parity with wordmark, rearrangeable tool block, and fixed administrative section (Settings, Plugins, Evals). Up/down controls persist ordering per user role while user/team card surfaces `Ivory v0.1.0a`.
- Document Review pipeline rebuilt: upload + custom prompt unified, active sessions separated, selecting a document triggers initial review summary before chat. Added workflow guidance outlining drop → auto review → chat → dashboard return.
- Research, Translation, and Evals views share a dashboard + workspace split with model selectors gated by provider availability. Each tool surfaces history to resume threads/jobs and clarifies that hosted models require stored API keys.
- Settings now manages provider secrets via add/remove flow (with SQLCipher storage note and remote-data disclaimer), separates template library vs workspace storage, supports dual sidebar positioning, and documents Elefant-hosted/custom sync endpoints.
- Pending integration hooks: provider availability should read from real secrets store, document review summaries need backend invoke, translation/evals exports require wiring, and sync test button must call health endpoint.
