import { createRouter, createWebHistory } from "vue-router";
import LoginPage from "@/pages/LoginPage.vue";
import PrListPage from "@/pages/PrListPage.vue";
import PrDetailPage from "@/pages/PrDetailPage.vue";
import IssueListPage from "@/pages/IssueListPage.vue";
import IssueNewPage from "@/pages/IssueNewPage.vue";
import SettingsPage from "@/pages/SettingsPage.vue";
import { useAuthStore } from "@/stores/useAuthStore";
import { authHasAnyToken, authHasToken } from "@/api";
import type { Platform } from "@/types";

const routes = [
  {
    path: "/",
    redirect: "/pr",
  },
  {
    path: "/login",
    name: "login",
    component: LoginPage,
  },
  {
    path: "/pr",
    name: "pr-list",
    component: PrListPage,
    meta: { requiresAuth: true },
  },
  {
    path: "/pr/:platform/:owner/:repo/:number",
    name: "pr-detail",
    component: PrDetailPage,
    props: true,
    meta: { requiresAuth: true },
  },
  {
    path: "/issue",
    name: "issue-list",
    component: IssueListPage,
    meta: { requiresAuth: true },
  },
  {
    path: "/issue/new",
    name: "issue-new",
    component: IssueNewPage,
    meta: { requiresAuth: true },
  },
  {
    path: "/settings",
    name: "settings",
    component: SettingsPage,
  },
];

const router = createRouter({
  history: createWebHistory(),
  routes,
});

router.beforeEach(async (to, _from, next) => {
  const store = useAuthStore();
  const platform: Platform | undefined =
    (to.params.platform as Platform | undefined) ?? store.activePlatform;
  const hasToken = platform ? await authHasToken(platform) : await authHasAnyToken();

  if (to.path === "/login" && hasToken && store.isLoggedIn) {
    next("/pr");
  } else if (to.meta.requiresAuth && !hasToken) {
    next("/login");
  } else {
    next();
  }
});

export default router;
