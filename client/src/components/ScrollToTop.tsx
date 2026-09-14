import { useEffect } from 'react';
import { useLocation } from 'react-router-dom';

/** Scroll to top on route change so footer clicks open the page focused at the top. */
export function ScrollToTop() {
  const { pathname } = useLocation();

  useEffect(() => {
    window.scrollTo(0, 0);
  }, [pathname]);

  return null;
}
