// ABOUTME: Shared reactive user info used in SidebarNav and TopBar.
// ABOUTME: Single source of truth for current user identity and app version.
import { ref } from "vue";

export const currentUser = ref({
  name: "Local User",
  team: "",
  version: "Rumble v0.1.0a",
});
