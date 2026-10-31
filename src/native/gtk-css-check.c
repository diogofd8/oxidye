/* Finite native CSS validation; no display connection or desktop settings. */
#include <gtk/gtk.h>
#include <stdio.h>
static int errors;
static void parse_error(GtkCssProvider *provider, GtkCssSection *section, GError *error, gpointer unused) {
  (void)provider;
  (void)section;
  (void)unused;
#if GTK_MAJOR_VERSION == 4
  if (error->domain == GTK_CSS_PARSER_WARNING && error->code == GTK_CSS_PARSER_WARNING_DEPRECATED)
    return;
#endif
  fprintf(stderr, "CSS: %s\n", error->message);
  errors++;
}
int main(int argc, char **argv) {
  if (argc != 2)
    return 2;
  GtkCssProvider *provider = gtk_css_provider_new();
  g_signal_connect(provider, "parsing-error", G_CALLBACK(parse_error), NULL);
#if GTK_MAJOR_VERSION == 3
  GError *error = NULL;
  gtk_css_provider_load_from_path(provider, argv[1], &error);
  if (error) {
    if (!errors)
      fprintf(stderr, "CSS: %s\n", error->message);
    errors++;
    g_error_free(error);
  }
#else
  gtk_css_provider_load_from_path(provider, argv[1]);
#endif
  g_object_unref(provider);
  return errors ? 1 : 0;
}
