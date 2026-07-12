import { createRouter, createWebHistory } from "vue-router";
import LoginPage from "@/pages/LoginPage.vue";
import PrListPage from "@/pages/PrListPage.vue";
import PrDetailPage from "@/pages/PrDetailPage.vue";
import IssueListPage from "@/pages/IssueListPage.vue";
import IssueNewPage from "@/pages/IssueNewPage.vue";
import SettingsPage from "@/pages/SettingsPage.vue";
import { useAuthStore } from "@/stores/useAuthStore";
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

function parsePlatform(value: unknown): Platform | undefined {
  return value === "github" || value === "gitlab" || value === "gitee" ? value : undefined;
}

router.beforeEach(async (to, _from, next) => {
  const store = useAuthStore();
  const routePlatform = parsePlatform(to.params.platform);
  const loginPlatform = to.path === "/login" ? parsePlatform(to.query.platform) : undefined;
  const targetPlatform = routePlatform ?? loginPlatform;
  const requiresAuthentication = to.path === "/login" || Boolean(to.meta.requiresAuth);

  if (loginPlatform) {
    store.setActivePlatform(loginPlatform);
  }

  let isLoggedIn = targetPlatform
    ? (store.platforms[targetPlatform]?.isLoggedIn ?? false)
    : store.isLoggedIn;

  if (requiresAuthentication && !isLoggedIn) {
    if (loginPlatform) {
      await store.restorePlatformSession(loginPlatform);
    } else {
      await store.restoreSession(routePlatform);
    }
    isLoggedIn = targetPlatform
      ? (store.platforms[targetPlatform]?.isLoggedIn ?? false)
      : store.isLoggedIn;
  }

  if (to.path === "/login" && isLoggedIn) {
    next("/pr");
  } else if (to.meta.requiresAuth && !isLoggedIn) {
    next("/login");
  } else {
    next();
  }
});

export default router;
