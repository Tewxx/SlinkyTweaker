#include <windows.h>
#include <GL/gl.h>
#include <stddef.h>

typedef char GLchar;
typedef ptrdiff_t GLsizeiptr;
typedef ptrdiff_t GLintptr;

#define GL_CLAMP_TO_EDGE 0x812F
#define GL_GENERATE_MIPMAP 0x8191
#define GL_INCR_WRAP 0x8507
#define GL_DECR_WRAP 0x8508
#define GL_TEXTURE0 0x84C0
#define GL_ACTIVE_TEXTURE 0x84E0
#define GL_ARRAY_BUFFER 0x8892
#define GL_ARRAY_BUFFER_BINDING 0x8894
#define GL_STREAM_DRAW 0x88E0
#define GL_FRAGMENT_SHADER 0x8B30
#define GL_VERTEX_SHADER 0x8B31
#define GL_COMPILE_STATUS 0x8B81
#define GL_LINK_STATUS 0x8B82
#define GL_CURRENT_PROGRAM 0x8B8D
#define GL_RED 0x1903
#define GL_R8 0x8229

#define NVG_GL_PROCS(X)                                                                     \
    X(void, glActiveTexture, (GLenum))                                                      \
    X(void, glAttachShader, (GLuint, GLuint))                                               \
    X(void, glBindAttribLocation, (GLuint, GLuint, const GLchar*))                          \
    X(void, glBindBuffer, (GLenum, GLuint))                                                 \
    X(void, glBlendFuncSeparate, (GLenum, GLenum, GLenum, GLenum))                          \
    X(void, glBufferData, (GLenum, GLsizeiptr, const void*, GLenum))                        \
    X(void, glCompileShader, (GLuint))                                                      \
    X(GLuint, glCreateProgram, (void))                                                      \
    X(GLuint, glCreateShader, (GLenum))                                                     \
    X(void, glDeleteBuffers, (GLsizei, const GLuint*))                                      \
    X(void, glDeleteProgram, (GLuint))                                                      \
    X(void, glDeleteShader, (GLuint))                                                       \
    X(void, glDisableVertexAttribArray, (GLuint))                                           \
    X(void, glEnableVertexAttribArray, (GLuint))                                            \
    X(void, glGenBuffers, (GLsizei, GLuint*))                                               \
    X(void, glGetProgramInfoLog, (GLuint, GLsizei, GLsizei*, GLchar*))                      \
    X(void, glGetProgramiv, (GLuint, GLenum, GLint*))                                       \
    X(void, glGetShaderInfoLog, (GLuint, GLsizei, GLsizei*, GLchar*))                       \
    X(void, glGetShaderiv, (GLuint, GLenum, GLint*))                                        \
    X(GLint, glGetUniformLocation, (GLuint, const GLchar*))                                 \
    X(void, glLinkProgram, (GLuint))                                                        \
    X(void, glShaderSource, (GLuint, GLsizei, const GLchar* const*, const GLint*))          \
    X(void, glStencilOpSeparate, (GLenum, GLenum, GLenum, GLenum))                          \
    X(void, glUniform1i, (GLint, GLint))                                                    \
    X(void, glUniform2fv, (GLint, GLsizei, const GLfloat*))                                 \
    X(void, glUniform4fv, (GLint, GLsizei, const GLfloat*))                                 \
    X(void, glUseProgram, (GLuint))                                                         \
    X(void, glVertexAttribPointer, (GLuint, GLint, GLenum, GLboolean, GLsizei, const void*))

#define NVG_DECLARE(ret, name, args) \
    typedef ret(APIENTRY* PFN_##name) args; \
    static PFN_##name nvg_##name;
NVG_GL_PROCS(NVG_DECLARE)
#undef NVG_DECLARE

#define glActiveTexture nvg_glActiveTexture
#define glAttachShader nvg_glAttachShader
#define glBindAttribLocation nvg_glBindAttribLocation
#define glBindBuffer nvg_glBindBuffer
#define glBlendFuncSeparate nvg_glBlendFuncSeparate
#define glBufferData nvg_glBufferData
#define glCompileShader nvg_glCompileShader
#define glCreateProgram nvg_glCreateProgram
#define glCreateShader nvg_glCreateShader
#define glDeleteBuffers nvg_glDeleteBuffers
#define glDeleteProgram nvg_glDeleteProgram
#define glDeleteShader nvg_glDeleteShader
#define glDisableVertexAttribArray nvg_glDisableVertexAttribArray
#define glEnableVertexAttribArray nvg_glEnableVertexAttribArray
#define glGenBuffers nvg_glGenBuffers
#define glGetProgramInfoLog nvg_glGetProgramInfoLog
#define glGetProgramiv nvg_glGetProgramiv
#define glGetShaderInfoLog nvg_glGetShaderInfoLog
#define glGetShaderiv nvg_glGetShaderiv
#define glGetUniformLocation nvg_glGetUniformLocation
#define glLinkProgram nvg_glLinkProgram
#define glShaderSource nvg_glShaderSource
#define glStencilOpSeparate nvg_glStencilOpSeparate
#define glUniform1i nvg_glUniform1i
#define glUniform2fv nvg_glUniform2fv
#define glUniform4fv nvg_glUniform4fv
#define glUseProgram nvg_glUseProgram
#define glVertexAttribPointer nvg_glVertexAttribPointer

#include "nanovg/nanovg.h"
#define NANOVG_GL2_IMPLEMENTATION
#include "nanovg/nanovg_gl.h"

static void* proc(const char* name) {
    void* p = (void*)wglGetProcAddress(name);
    if (p == (void*)0 || p == (void*)1 || p == (void*)2 || p == (void*)3 || p == (void*)-1) {
        return NULL;
    }
    return p;
}

int nvgrs_load_gl(void) {
    int index = 0;
#define NVG_LOAD(ret, name, args)                       \
    index++;                                            \
    nvg_##name = (PFN_##name)proc(#name);               \
    if (!nvg_##name) return index;
    NVG_GL_PROCS(NVG_LOAD)
#undef NVG_LOAD
    return 0;
}

NVGcontext* nvgrs_create(int flags) { return nvgCreateGL2(flags); }
void nvgrs_delete(NVGcontext* ctx) { nvgDeleteGL2(ctx); }

typedef struct {
    GLint program;
    GLint array_buffer;
    GLint active_texture;
} NvgrsSaved;

void nvgrs_save(NvgrsSaved* saved) {
    glGetIntegerv(GL_CURRENT_PROGRAM, &saved->program);
    glGetIntegerv(GL_ARRAY_BUFFER_BINDING, &saved->array_buffer);
    glGetIntegerv(GL_ACTIVE_TEXTURE, &saved->active_texture);
    glPushAttrib(GL_ALL_ATTRIB_BITS);
    glPushClientAttrib(GL_CLIENT_ALL_ATTRIB_BITS);
    nvg_glActiveTexture(GL_TEXTURE0);
    glDisable(GL_ALPHA_TEST);
    glDisable(GL_DEPTH_TEST);
    glDisable(GL_SCISSOR_TEST);
}

void nvgrs_restore(const NvgrsSaved* saved) {
    nvg_glUseProgram((GLuint)saved->program);
    nvg_glBindBuffer(GL_ARRAY_BUFFER, (GLuint)saved->array_buffer);
    glPopClientAttrib();
    glPopAttrib();
    nvg_glActiveTexture((GLenum)saved->active_texture);
}

void nvgrs_begin(NVGcontext* ctx, NvgrsSaved* saved, float width, float height) {
    nvgrs_save(saved);
    glViewport(0, 0, (GLsizei)width, (GLsizei)height);
    nvgBeginFrame(ctx, width, height, 1.0f);
}

void nvgrs_end(NVGcontext* ctx, const NvgrsSaved* saved) {
    nvgEndFrame(ctx);
    nvgrs_restore(saved);
}

#define GL_RGBA8 0x8058
#define GL_FRAMEBUFFER 0x8D40
#define GL_READ_FRAMEBUFFER 0x8CA8
#define GL_DRAW_FRAMEBUFFER 0x8CA9
#define GL_READ_FRAMEBUFFER_BINDING 0x8CAA
#define GL_DRAW_FRAMEBUFFER_BINDING 0x8CA6
#define GL_COLOR_ATTACHMENT0 0x8CE0
#define GL_FRAMEBUFFER_COMPLETE 0x8CD5

#define BLUR_PROCS(X)                                                                        \
    X(void, glGenFramebuffers, (GLsizei, GLuint*))                                           \
    X(void, glBindFramebuffer, (GLenum, GLuint))                                             \
    X(void, glFramebufferTexture2D, (GLenum, GLenum, GLenum, GLuint, GLint))                 \
    X(void, glBlitFramebuffer, (GLint, GLint, GLint, GLint, GLint, GLint, GLint, GLint, GLbitfield, GLenum)) \
    X(void, glUniform2f, (GLint, GLfloat, GLfloat))                                           \
    X(GLenum, glCheckFramebufferStatus, (GLenum))

#define BLUR_DECLARE(ret, name, args) \
    typedef ret(APIENTRY* PFN_blur_##name) args; \
    static PFN_blur_##name blur_##name;
BLUR_PROCS(BLUR_DECLARE)
#undef BLUR_DECLARE

static const char* BLUR_VS =
    "#version 120\n"
    "attribute vec2 p;\n"
    "varying vec2 uv;\n"
    "void main() {\n"
    "    uv = p;\n"
    "    gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0);\n"
    "}\n";

static const char* BLUR_FS =
    "#version 120\n"
    "uniform sampler2D tex;\n"
    "uniform vec2 dir;\n"
    "varying vec2 uv;\n"
    "void main() {\n"
    "    vec3 c = texture2D(tex, uv).rgb * 0.2270270270;\n"
    "    c += (texture2D(tex, uv + dir * 1.3846153846).rgb + texture2D(tex, uv - dir * 1.3846153846).rgb) * 0.3162162162;\n"
    "    c += (texture2D(tex, uv + dir * 3.2307692308).rgb + texture2D(tex, uv - dir * 3.2307692308).rgb) * 0.0702702703;\n"
    "    gl_FragColor = vec4(c, 1.0);\n"
    "}\n";

#define BLUR_DOWNSCALE 4

static GLuint blur_program, blur_vbo, blur_fbo[3], blur_tex[3];
static GLint blur_dir_loc, blur_tex_loc;
static int blur_w, blur_h, blur_image, blur_failed;
static NVGcontext* blur_ctx;

static GLuint blur_shader(GLenum type, const char* src) {
    GLint ok = 0;
    GLuint s = nvg_glCreateShader(type);
    nvg_glShaderSource(s, 1, &src, NULL);
    nvg_glCompileShader(s);
    nvg_glGetShaderiv(s, GL_COMPILE_STATUS, &ok);
    if (!ok) {
        nvg_glDeleteShader(s);
        return 0;
    }
    return s;
}

static int blur_init(void) {
    static const GLfloat triangle[6] = {0.0f, 0.0f, 2.0f, 0.0f, 0.0f, 2.0f};
    GLint ok = 0, buffer = 0;
    GLuint vs, fs;
    if (blur_program) return 1;
    if (blur_failed) return 0;
    blur_failed = 1;
#define BLUR_LOAD(ret, name, args)                   \
    blur_##name = (PFN_blur_##name)proc(#name);      \
    if (!blur_##name) return 0;
    BLUR_PROCS(BLUR_LOAD)
#undef BLUR_LOAD
    vs = blur_shader(GL_VERTEX_SHADER, BLUR_VS);
    fs = blur_shader(GL_FRAGMENT_SHADER, BLUR_FS);
    if (!vs || !fs) {
        if (vs) nvg_glDeleteShader(vs);
        if (fs) nvg_glDeleteShader(fs);
        return 0;
    }
    blur_program = nvg_glCreateProgram();
    nvg_glAttachShader(blur_program, vs);
    nvg_glAttachShader(blur_program, fs);
    nvg_glBindAttribLocation(blur_program, 0, "p");
    nvg_glLinkProgram(blur_program);
    nvg_glDeleteShader(vs);
    nvg_glDeleteShader(fs);
    nvg_glGetProgramiv(blur_program, GL_LINK_STATUS, &ok);
    if (!ok) {
        nvg_glDeleteProgram(blur_program);
        blur_program = 0;
        return 0;
    }
    blur_dir_loc = nvg_glGetUniformLocation(blur_program, "dir");
    blur_tex_loc = nvg_glGetUniformLocation(blur_program, "tex");
    glGetIntegerv(GL_ARRAY_BUFFER_BINDING, &buffer);
    nvg_glGenBuffers(1, &blur_vbo);
    nvg_glBindBuffer(GL_ARRAY_BUFFER, blur_vbo);
    nvg_glBufferData(GL_ARRAY_BUFFER, sizeof(triangle), triangle, GL_STREAM_DRAW);
    nvg_glBindBuffer(GL_ARRAY_BUFFER, (GLuint)buffer);
    blur_glGenFramebuffers(3, blur_fbo);
    glGenTextures(3, blur_tex);
    blur_failed = 0;
    return 1;
}

static void blur_resize(NVGcontext* ctx, int w, int h) {
    int i;
    if (w == blur_w && h == blur_h && ctx == blur_ctx) return;
    blur_w = w;
    blur_h = h;
    for (i = 0; i < 3; i++) {
        glBindTexture(GL_TEXTURE_2D, blur_tex[i]);
        glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA8, w, h, 0, GL_RGBA, GL_UNSIGNED_BYTE, NULL);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
        blur_glBindFramebuffer(GL_FRAMEBUFFER, blur_fbo[i]);
        blur_glFramebufferTexture2D(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_TEXTURE_2D, blur_tex[i], 0);
    }
    if (blur_image && ctx == blur_ctx) nvgDeleteImage(ctx, blur_image);
    blur_ctx = ctx;
    blur_image = nvglCreateImageFromHandleGL2(ctx, blur_tex[0], w, h, NVG_IMAGE_FLIPY | NVG_IMAGE_NODELETE);
}

static void blur_pass(int from, int to, float dx, float dy) {
    blur_glBindFramebuffer(GL_FRAMEBUFFER, blur_fbo[to]);
    glBindTexture(GL_TEXTURE_2D, blur_tex[from]);
    blur_glUniform2f(blur_dir_loc, dx, dy);
    glDrawArrays(GL_TRIANGLES, 0, 3);
}

int nvgrs_blur(NVGcontext* ctx, int width, int height, int rx, int ry, int rw, int rh, int passes) {
    GLint draw_fbo = 0, read_fbo = 0, program = 0, buffer = 0;
    int w = width / BLUR_DOWNSCALE, h = height / BLUR_DOWNSCALE, i;
    int x0, y0, x1, y1, dx0, dy0, dx1, dy1;
    if (w < 1 || h < 1 || !blur_init()) return 0;
    x0 = rx < 0 ? 0 : rx;
    y0 = ry < 0 ? 0 : ry;
    x1 = rx + rw > width ? width : rx + rw;
    y1 = ry + rh > height ? height : ry + rh;
    if (x1 <= x0 || y1 <= y0) return 0;
    dx0 = x0 / BLUR_DOWNSCALE;
    dx1 = (x1 + BLUR_DOWNSCALE - 1) / BLUR_DOWNSCALE;
    dy0 = (height - y1) / BLUR_DOWNSCALE;
    dy1 = (height - y0 + BLUR_DOWNSCALE - 1) / BLUR_DOWNSCALE;
    if (dx1 > w) dx1 = w;
    if (dy1 > h) dy1 = h;
    if (dx1 <= dx0 || dy1 <= dy0) return 0;
    glGetIntegerv(GL_DRAW_FRAMEBUFFER_BINDING, &draw_fbo);
    glGetIntegerv(GL_READ_FRAMEBUFFER_BINDING, &read_fbo);
    glGetIntegerv(GL_CURRENT_PROGRAM, &program);
    glGetIntegerv(GL_ARRAY_BUFFER_BINDING, &buffer);
    glPushAttrib(GL_ALL_ATTRIB_BITS);
    glPushClientAttrib(GL_CLIENT_ALL_ATTRIB_BITS);
    blur_resize(ctx, w, h);

    glDisable(GL_SCISSOR_TEST);
    glDisable(GL_BLEND);
    glDisable(GL_DEPTH_TEST);
    glDisable(GL_STENCIL_TEST);
    glDisable(GL_CULL_FACE);
    glDisable(GL_ALPHA_TEST);
    glColorMask(GL_TRUE, GL_TRUE, GL_TRUE, GL_TRUE);

    blur_glBindFramebuffer(GL_READ_FRAMEBUFFER, (GLuint)read_fbo);
    blur_glBindFramebuffer(GL_DRAW_FRAMEBUFFER, blur_fbo[2]);
    blur_glBlitFramebuffer(dx0 * BLUR_DOWNSCALE, dy0 * BLUR_DOWNSCALE, dx1 * BLUR_DOWNSCALE, dy1 * BLUR_DOWNSCALE,
                           dx0, dy0, dx1, dy1, GL_COLOR_BUFFER_BIT, GL_LINEAR);
    blur_glBindFramebuffer(GL_READ_FRAMEBUFFER, blur_fbo[2]);
    blur_glBindFramebuffer(GL_DRAW_FRAMEBUFFER, blur_fbo[0]);
    blur_glBlitFramebuffer(dx0, dy0, dx1, dy1, dx0, dy0, dx1, dy1, GL_COLOR_BUFFER_BIT, GL_NEAREST);

    glViewport(0, 0, w, h);
    glEnable(GL_SCISSOR_TEST);
    glScissor(dx0, dy0, dx1 - dx0, dy1 - dy0);
    nvg_glUseProgram(blur_program);
    nvg_glUniform1i(blur_tex_loc, 0);
    nvg_glActiveTexture(GL_TEXTURE0);
    nvg_glBindBuffer(GL_ARRAY_BUFFER, blur_vbo);
    nvg_glEnableVertexAttribArray(0);
    nvg_glVertexAttribPointer(0, 2, GL_FLOAT, GL_FALSE, 0, (const void*)0);
    for (i = 0; i < passes; i++) {
        float spread = 1.0f + (float)i * 0.5f;
        blur_pass(0, 1, spread / (float)w, 0.0f);
        blur_pass(1, 0, 0.0f, spread / (float)h);
    }
    nvg_glDisableVertexAttribArray(0);

    blur_glBindFramebuffer(GL_DRAW_FRAMEBUFFER, (GLuint)draw_fbo);
    blur_glBindFramebuffer(GL_READ_FRAMEBUFFER, (GLuint)read_fbo);
    nvg_glUseProgram((GLuint)program);
    nvg_glBindBuffer(GL_ARRAY_BUFFER, (GLuint)buffer);
    glPopClientAttrib();
    glPopAttrib();
    return blur_image;
}

void nvgrs_fill_image(NVGcontext* ctx, float x, float y, float w, float h, int image, float alpha) {
    nvgFillPaint(ctx, nvgImagePattern(ctx, x, y, w, h, 0.0f, image, alpha));
    nvgFill(ctx);
}

static const char* GLOW_FS =
    "#version 120\n"
    "uniform sampler2D tex;\n"
    "uniform vec2 dir;\n"
    "varying vec2 uv;\n"
    "void main() {\n"
    "    vec4 c = texture2D(tex, uv) * 0.2270270270;\n"
    "    c += (texture2D(tex, uv + dir * 1.3846153846) + texture2D(tex, uv - dir * 1.3846153846)) * 0.3162162162;\n"
    "    c += (texture2D(tex, uv + dir * 3.2307692308) + texture2D(tex, uv - dir * 3.2307692308)) * 0.0702702703;\n"
    "    gl_FragColor = c;\n"
    "}\n";

static GLuint layer_fbo, layer_tex, glow_program, glow_fbo, glow_tex;
static GLint glow_dir_loc, glow_tex_loc;
static int layer_w, layer_h, layer_image, glow_w, glow_h, glow_failed;
static NVGcontext* layer_ctx;
static GLint layer_draw_fbo, layer_read_fbo;
static GLfloat layer_clear[4];

static int glow_init(void) {
    GLint ok = 0;
    GLuint vs, fs;
    if (glow_program) return 1;
    if (glow_failed || !blur_init()) return 0;
    glow_failed = 1;
    vs = blur_shader(GL_VERTEX_SHADER, BLUR_VS);
    fs = blur_shader(GL_FRAGMENT_SHADER, GLOW_FS);
    if (!vs || !fs) {
        if (vs) nvg_glDeleteShader(vs);
        if (fs) nvg_glDeleteShader(fs);
        return 0;
    }
    glow_program = nvg_glCreateProgram();
    nvg_glAttachShader(glow_program, vs);
    nvg_glAttachShader(glow_program, fs);
    nvg_glBindAttribLocation(glow_program, 0, "p");
    nvg_glLinkProgram(glow_program);
    nvg_glDeleteShader(vs);
    nvg_glDeleteShader(fs);
    nvg_glGetProgramiv(glow_program, GL_LINK_STATUS, &ok);
    if (!ok) {
        nvg_glDeleteProgram(glow_program);
        glow_program = 0;
        return 0;
    }
    glow_dir_loc = nvg_glGetUniformLocation(glow_program, "dir");
    glow_tex_loc = nvg_glGetUniformLocation(glow_program, "tex");
    blur_glGenFramebuffers(1, &layer_fbo);
    blur_glGenFramebuffers(1, &glow_fbo);
    glGenTextures(1, &layer_tex);
    glGenTextures(1, &glow_tex);
    glow_failed = 0;
    return 1;
}

static void glow_texture(GLuint tex, GLuint fbo, int w, int h) {
    glBindTexture(GL_TEXTURE_2D, tex);
    glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA8, w, h, 0, GL_RGBA, GL_UNSIGNED_BYTE, NULL);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
    blur_glBindFramebuffer(GL_FRAMEBUFFER, fbo);
    blur_glFramebufferTexture2D(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_TEXTURE_2D, tex, 0);
}

int nvgrs_layer_begin(NVGcontext* ctx, NvgrsSaved* saved, int w, int h) {
    if (w < 1 || h < 1 || !glow_init()) return 0;
    glGetIntegerv(GL_DRAW_FRAMEBUFFER_BINDING, &layer_draw_fbo);
    glGetIntegerv(GL_READ_FRAMEBUFFER_BINDING, &layer_read_fbo);
    glGetFloatv(GL_COLOR_CLEAR_VALUE, layer_clear);
    nvgrs_save(saved);
    if (layer_w != w || layer_h != h || layer_ctx != ctx) {
        layer_w = w;
        layer_h = h;
        glow_texture(layer_tex, layer_fbo, w, h);
        if (layer_image && layer_ctx == ctx) nvgDeleteImage(ctx, layer_image);
        layer_ctx = ctx;
        layer_image = nvglCreateImageFromHandleGL2(ctx, layer_tex, w, h,
                                                   NVG_IMAGE_FLIPY | NVG_IMAGE_PREMULTIPLIED | NVG_IMAGE_NODELETE);
    }
    blur_glBindFramebuffer(GL_FRAMEBUFFER, layer_fbo);
    if (!layer_image || blur_glCheckFramebufferStatus(GL_FRAMEBUFFER) != GL_FRAMEBUFFER_COMPLETE) {
        blur_glBindFramebuffer(GL_DRAW_FRAMEBUFFER, (GLuint)layer_draw_fbo);
        blur_glBindFramebuffer(GL_READ_FRAMEBUFFER, (GLuint)layer_read_fbo);
        nvgrs_restore(saved);
        return 0;
    }
    glViewport(0, 0, w, h);
    glColorMask(GL_TRUE, GL_TRUE, GL_TRUE, GL_TRUE);
    glClearColor(0, 0, 0, 0);
    glClear(GL_COLOR_BUFFER_BIT);
    nvgBeginFrame(ctx, (float)w, (float)h, 1.0f);
    return layer_image;
}

void nvgrs_layer_end(NVGcontext* ctx, const NvgrsSaved* saved) {
    nvgEndFrame(ctx);
    blur_glBindFramebuffer(GL_DRAW_FRAMEBUFFER, (GLuint)layer_draw_fbo);
    blur_glBindFramebuffer(GL_READ_FRAMEBUFFER, (GLuint)layer_read_fbo);
    glClearColor(layer_clear[0], layer_clear[1], layer_clear[2], layer_clear[3]);
    nvgrs_restore(saved);
}

int nvgrs_layer_blur(int passes) {
    GLint draw_fbo = 0, read_fbo = 0, program = 0, buffer = 0;
    int i;
    if (!layer_fbo || layer_w < 1 || layer_h < 1 || !glow_init()) return 0;
    glGetIntegerv(GL_DRAW_FRAMEBUFFER_BINDING, &draw_fbo);
    glGetIntegerv(GL_READ_FRAMEBUFFER_BINDING, &read_fbo);
    glGetIntegerv(GL_CURRENT_PROGRAM, &program);
    glGetIntegerv(GL_ARRAY_BUFFER_BINDING, &buffer);
    glPushAttrib(GL_ALL_ATTRIB_BITS);
    glPushClientAttrib(GL_CLIENT_ALL_ATTRIB_BITS);
    if (glow_w != layer_w || glow_h != layer_h) {
        glow_w = layer_w;
        glow_h = layer_h;
        glow_texture(glow_tex, glow_fbo, glow_w, glow_h);
    }
    glDisable(GL_SCISSOR_TEST);
    glDisable(GL_BLEND);
    glDisable(GL_DEPTH_TEST);
    glDisable(GL_STENCIL_TEST);
    glDisable(GL_CULL_FACE);
    glDisable(GL_ALPHA_TEST);
    glColorMask(GL_TRUE, GL_TRUE, GL_TRUE, GL_TRUE);
    glViewport(0, 0, layer_w, layer_h);
    nvg_glUseProgram(glow_program);
    nvg_glUniform1i(glow_tex_loc, 0);
    nvg_glActiveTexture(GL_TEXTURE0);
    nvg_glBindBuffer(GL_ARRAY_BUFFER, blur_vbo);
    nvg_glEnableVertexAttribArray(0);
    nvg_glVertexAttribPointer(0, 2, GL_FLOAT, GL_FALSE, 0, (const void*)0);
    for (i = 0; i < passes; i++) {
        float spread = 1.0f + (float)i;
        blur_glBindFramebuffer(GL_FRAMEBUFFER, glow_fbo);
        glBindTexture(GL_TEXTURE_2D, layer_tex);
        blur_glUniform2f(glow_dir_loc, spread / (float)layer_w, 0.0f);
        glDrawArrays(GL_TRIANGLES, 0, 3);
        blur_glBindFramebuffer(GL_FRAMEBUFFER, layer_fbo);
        glBindTexture(GL_TEXTURE_2D, glow_tex);
        blur_glUniform2f(glow_dir_loc, 0.0f, spread / (float)layer_h);
        glDrawArrays(GL_TRIANGLES, 0, 3);
    }
    nvg_glDisableVertexAttribArray(0);
    blur_glBindFramebuffer(GL_DRAW_FRAMEBUFFER, (GLuint)draw_fbo);
    blur_glBindFramebuffer(GL_READ_FRAMEBUFFER, (GLuint)read_fbo);
    nvg_glUseProgram((GLuint)program);
    nvg_glBindBuffer(GL_ARRAY_BUFFER, (GLuint)buffer);
    glPopClientAttrib();
    glPopAttrib();
    return 1;
}
