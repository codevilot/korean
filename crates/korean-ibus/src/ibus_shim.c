#include <ibus.h>
#include <stdint.h>
#include <stdlib.h>

typedef struct _BkHangulEngine {
    IBusEngine parent;
    void *state;
} BkHangulEngine;

typedef struct _BkHangulEngineClass {
    IBusEngineClass parent;
} BkHangulEngineClass;

extern void *bk_engine_state_new(void);
extern void bk_engine_state_free(void *state);
extern int bk_engine_process_key_event(void *state,
                                       IBusEngine *engine,
                                       uint32_t keyval,
                                       uint32_t keycode,
                                       uint32_t modifiers);
extern void bk_engine_reset_state(void *state, IBusEngine *engine);

G_DEFINE_TYPE(BkHangulEngine, korean_engine, IBUS_TYPE_ENGINE)

void bk_ibus_commit_text(IBusEngine *engine, const char *text) {
    IBusText *ibus_text = ibus_text_new_from_string(text);
    ibus_engine_commit_text(engine, ibus_text);
}

void bk_ibus_update_preedit(IBusEngine *engine,
                            const char *text,
                            uint32_t cursor_pos,
                            int visible) {
    IBusText *ibus_text = ibus_text_new_from_string(text);
    ibus_engine_update_preedit_text(engine, ibus_text, cursor_pos, visible);
}

void bk_ibus_hide_preedit(IBusEngine *engine) {
    IBusText *ibus_text = ibus_text_new_from_static_string("");
    ibus_engine_update_preedit_text(engine, ibus_text, 0, FALSE);
    ibus_engine_hide_preedit_text(engine);
}

static gboolean korean_engine_process_key_event(IBusEngine *engine,
                                                   guint keyval,
                                                   guint keycode,
                                                   guint modifiers) {
    BkHangulEngine *bk_engine = (BkHangulEngine *)engine;
    return bk_engine_process_key_event(
        bk_engine->state, engine, keyval, keycode, modifiers);
}

static void korean_engine_reset(IBusEngine *engine) {
    BkHangulEngine *bk_engine = (BkHangulEngine *)engine;
    bk_engine_reset_state(bk_engine->state, engine);
}

static void korean_engine_focus_out(IBusEngine *engine) {
    BkHangulEngine *bk_engine = (BkHangulEngine *)engine;
    bk_engine_reset_state(bk_engine->state, engine);
}

static void korean_engine_finalize(GObject *object) {
    BkHangulEngine *bk_engine = (BkHangulEngine *)object;
    if (bk_engine->state != NULL) {
        bk_engine_state_free(bk_engine->state);
        bk_engine->state = NULL;
    }
    G_OBJECT_CLASS(korean_engine_parent_class)->finalize(object);
}

static void korean_engine_init(BkHangulEngine *engine) {
    engine->state = bk_engine_state_new();
}

static void korean_engine_class_init(BkHangulEngineClass *klass) {
    IBusEngineClass *engine_class = IBUS_ENGINE_CLASS(klass);
    GObjectClass *object_class = G_OBJECT_CLASS(klass);

    engine_class->process_key_event = korean_engine_process_key_event;
    engine_class->reset = korean_engine_reset;
    engine_class->focus_out = korean_engine_focus_out;
    object_class->finalize = korean_engine_finalize;
}

int bk_ibus_run(void) {
    ibus_init();

    IBusBus *bus = ibus_bus_new();
    if (!ibus_bus_is_connected(bus)) {
        g_printerr("korean: could not connect to ibus-daemon\n");
        g_object_unref(bus);
        return 1;
    }

    IBusFactory *factory = ibus_factory_new(ibus_bus_get_connection(bus));
    ibus_factory_add_engine(factory, "korean", korean_engine_get_type());
    ibus_factory_add_engine(factory, "korean-dev", korean_engine_get_type());

    const char *service_name = getenv("KOREAN_IBUS_SERVICE");
    if (service_name == NULL || service_name[0] == '\0') {
        service_name = "org.freedesktop.IBus.Korean";
    }

    guint32 reply = ibus_bus_request_name(bus, service_name, 0);
    if (reply == 0) {
        g_printerr("korean: failed to request IBus service name\n");
        g_object_unref(factory);
        g_object_unref(bus);
        return 1;
    }

    ibus_main();

    g_object_unref(factory);
    g_object_unref(bus);
    return 0;
}
