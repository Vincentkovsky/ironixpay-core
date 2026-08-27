import type { Router } from 'vue-router';
import setupUserLoginInfoGuard from './userLoginInfo';

export default function createRouteGuard(router: Router) {
    setupUserLoginInfoGuard(router);
}
