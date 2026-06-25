/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `NSURLRequest and NSMutableURLRequest`.

use super::{ns_string, NSTimeInterval, NSUInteger};
use crate::frameworks::foundation::ns_string::to_rust_string;
use crate::objc::{
    autorelease, id, msg, msg_class, nil, objc_classes, release, retain, ClassExports, HostObject,
    NSZonePtr,
};

type NSURLRequestCachePolicy = NSUInteger;
const NSURLRequestUseProtocolCachePolicy: NSURLRequestCachePolicy = 0;
const NSURLRequestReloadIgnoringLocalCache: NSURLRequestCachePolicy = 1;
#[allow(dead_code)]
const NSURLRequestReloadIgnoringCacheData: NSURLRequestCachePolicy = 1; // alias
const NSURLRequestReturnCacheDataElseLoad: NSURLRequestCachePolicy = 2;
const NSURLRequestReturnCacheDataDontLoad: NSURLRequestCachePolicy = 3;
const NSURLRequestReloadRevalidatingCacheData: NSURLRequestCachePolicy = 4;

type NSURLRequestNetworkServiceType = NSUInteger;
const NSURLNetworkServiceTypeDefault: NSURLRequestNetworkServiceType = 0;
#[allow(dead_code)]
const NSURLNetworkServiceTypeVoIP: NSURLRequestNetworkServiceType = 1;
#[allow(dead_code)]
const NSURLNetworkServiceTypeVideo: NSURLRequestNetworkServiceType = 2;
#[allow(dead_code)]
const NSURLNetworkServiceTypeBackground: NSURLRequestNetworkServiceType = 3;
#[allow(dead_code)]
const NSURLNetworkServiceTypeVoice: NSURLRequestNetworkServiceType = 4;

#[derive(Default)]
struct NSURLRequestHostObject {
    url: id,
    main_document_url: id,
    cache_policy: NSURLRequestCachePolicy,
    timeout_interval: NSTimeInterval,
    network_service_type: NSURLRequestNetworkServiceType,
    allows_cellular_access: bool,
    handles_cookies: bool,
    http_method: id,
    http_method_is_owned: bool,
    http_body: id,
    http_body_stream: id,
    http_should_handle_cookies: bool,
    http_should_use_pipelining: bool,
    http_header_fields: id,
}
impl HostObject for NSURLRequestHostObject {}

// --- Kısırdöngü Engelleyici Turnike Mekanizması ---
std::thread_local! {
    static REQUEST_DEPTH: std::cell::Cell<usize> = std::cell::Cell::new(0);
}

struct DepthGuard;
impl DepthGuard {
    fn new() -> Self {
        REQUEST_DEPTH.with(|d| d.set(d.get() + 1));
        Self
    }
}
impl Drop for DepthGuard {
    fn drop(&mut self) {
        REQUEST_DEPTH.with(|d| d.set(d.get() - 1));
    }
}
// --------------------------------------------------

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation NSURLRequest: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let http_header_fields: id = msg_class![env; NSMutableDictionary new];
    let host_object = Box::new(NSURLRequestHostObject {
        url: nil,
        main_document_url: nil,
        cache_policy: NSURLRequestUseProtocolCachePolicy,
        timeout_interval: 60.0,
        network_service_type: NSURLNetworkServiceTypeDefault,
        allows_cellular_access: true,
        handles_cookies: true,
        http_method: ns_string::get_static_str(env, "GET"),
        http_method_is_owned: false,
        http_body: nil,
        http_body_stream: nil,
        http_should_handle_cookies: true,
        http_should_use_pipelining: false,
        http_header_fields,
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

// MARK: - Constructors

+ (id)requestWithURL:(id)url {
    msg![env; this requestWithURL:url
                      cachePolicy:NSURLRequestUseProtocolCachePolicy
                  timeoutInterval:60.0]
}

+ (id)requestWithURL:(id)url
         cachePolicy:(NSURLRequestCachePolicy)cache_policy
     timeoutInterval:(NSTimeInterval)timeout_interval {
    // Oyun içi paralel istekleri desteklemek için limit 32'ye çıkarıldı
    let current_depth = REQUEST_DEPTH.with(|d| d.get());
    if current_depth > 32 {
        log_dbg!("⚠️ [Korumalı] NSURLRequest requestWithURL üst üste çok derin çağrıldı, kısırdöngü kırılıyor.");
        return nil;
    }
    let _guard = DepthGuard::new();

    let new: id = msg![env; this alloc];
    let new: id = msg![env; new initWithURL:url
                                cachePolicy:cache_policy
                            timeoutInterval:timeout_interval];
    autorelease(env, new)
}

- (id)initWithURL:(id)url {
    msg![env; this initWithURL:url
                   cachePolicy:NSURLRequestUseProtocolCachePolicy
               timeoutInterval:60.0]
}

- (id)initWithURL:(id)url
      cachePolicy:(NSURLRequestCachePolicy)cache_policy
  timeoutInterval:(NSTimeInterval)timeout_interval {
    let current_depth = REQUEST_DEPTH.with(|d| d.get());
    if current_depth > 32 {
        log_dbg!("⚠️ [Korumalı] NSURLRequest initWithURL üst üste çok derin çağrıldı, kısırdöngü kırılıyor.");
        release(env, this);
        return nil;
    }
    let _guard = DepthGuard::new();

    if url == nil {
        release(env, this);
        return nil;
    }

    let url_copy: id = msg![env; url copy];
    {
        let host = env.objc.borrow_mut::<NSURLRequestHostObject>(this);
        host.url              = url_copy;
        host.cache_policy     = cache_policy;
        host.timeout_interval = timeout_interval;
    }
    this
}

// MARK: - Dealloc

- (())dealloc {
    log_dbg!("[(NSURLRequest*){:?} dealloc]", this);
    
    // Güvenli Ayrıştırma: borrow guard'ı izole bir blokta bitiriyoruz.
    let (url, main_document_url, http_body, http_body_stream, http_header_fields, http_method) = {
        let host = env.objc.borrow::<NSURLRequestHostObject>(this);
        (
            host.url,
            host.main_document_url,
            host.http_body,
            host.http_body_stream,
            host.http_header_fields,
            if host.http_method_is_owned { host.http_method } else { nil }
        )
    }; // host borrow burada drop oldu. dealloc_object artık güvenli.

    release(env, url);
    release(env, main_document_url);
    release(env, http_method);
    release(env, http_body);
    release(env, http_body_stream);
    release(env, http_header_fields);
    env.objc.dealloc_object(this, &mut env.mem)
}

// MARK: - copy / mutableCopy

- (id)copy {
    let new: id = msg_class![env; NSURLRequest alloc];
    
    // Kilit çakışmasını önlemek için verileri izole blokta kopyalıyoruz
    let (url, cp, ti, nst, aca, shc, sup, method, method_owned, body, body_stream, src_headers) = {
        let host = env.objc.borrow::<NSURLRequestHostObject>(this);
        (
            host.url, host.cache_policy, host.timeout_interval, host.network_service_type,
            host.allows_cellular_access, host.http_should_handle_cookies, host.http_should_use_pipelining,
            host.http_method, host.http_method_is_owned, host.http_body, host.http_body_stream, host.http_header_fields
        )
    }; // 'this' kilidi açıldı.

    let new: id = msg![env; new initWithURL:url cachePolicy:cp timeoutInterval:ti];
    if new == nil { return nil; }

    {
        let h = env.objc.borrow_mut::<NSURLRequestHostObject>(new);
        h.network_service_type     = nst;
        h.allows_cellular_access   = aca;
        h.http_should_handle_cookies = shc;
        h.http_should_use_pipelining = sup;
    }

    if method_owned && method != nil {
        let method_copy: id = msg![env; method copy];
        let h = env.objc.borrow_mut::<NSURLRequestHostObject>(new);
        h.http_method = method_copy;
        h.http_method_is_owned = true;
    }

    if body != nil {
        let body_copy: id = msg![env; body copy];
        env.objc.borrow_mut::<NSURLRequestHostObject>(new).http_body = body_copy;
    }

    if body_stream != nil {
        retain(env, body_stream);
        env.objc.borrow_mut::<NSURLRequestHostObject>(new).http_body_stream = body_stream;
    }

    if src_headers != nil {
        let old_headers = env.objc.borrow::<NSURLRequestHostObject>(new).http_header_fields;
        release(env, old_headers);
        let headers_copy: id = msg![env; src_headers mutableCopy];
        env.objc.borrow_mut::<NSURLRequestHostObject>(new).http_header_fields = headers_copy;
    }

    new
}

- (id)mutableCopy {
    let new: id = msg_class![env; NSMutableURLRequest alloc];
    
    let (url, cp, ti, nst, aca, shc, sup, method, method_owned, body, body_stream, src_headers) = {
        let host = env.objc.borrow::<NSURLRequestHostObject>(this);
        (
            host.url, host.cache_policy, host.timeout_interval, host.network_service_type,
            host.allows_cellular_access, host.http_should_handle_cookies, host.http_should_use_pipelining,
            host.http_method, host.http_method_is_owned, host.http_body, host.http_body_stream, host.http_header_fields
        )
    }; // 'this' kilidi açıldı.

    let new: id = msg![env; new initWithURL:url cachePolicy:cp timeoutInterval:ti];
    if new == nil { return nil; }

    {
        let h = env.objc.borrow_mut::<NSURLRequestHostObject>(new);
        h.network_service_type       = nst;
        h.allows_cellular_access     = aca;
        h.http_should_handle_cookies = shc;
        h.http_should_use_pipelining = sup;
    }

    if method_owned && method != nil {
        let method_copy: id = msg![env; method copy];
        let h = env.objc.borrow_mut::<NSURLRequestHostObject>(new);
        h.http_method = method_copy;
        h.http_method_is_owned = true;
    }

    if body != nil {
        let body_copy: id = msg![env; body copy];
        env.objc.borrow_mut::<NSURLRequestHostObject>(new).http_body = body_copy;
    }

    if body_stream != nil {
        retain(env, body_stream);
        env.objc.borrow_mut::<NSURLRequestHostObject>(new).http_body_stream = body_stream;
    }

    if src_headers != nil {
        let old_headers = env.objc.borrow::<NSURLRequestHostObject>(new).http_header_fields;
        release(env, old_headers);
        let headers_copy: id = msg![env; src_headers mutableCopy];
        env.objc.borrow_mut::<NSURLRequestHostObject>(new).http_header_fields = headers_copy;
    }

    new
}

// MARK: - URL / policy accessors

- (id)URL {
    env.objc.borrow::<NSURLRequestHostObject>(this).url
}

- (id)mainDocumentURL {
    env.objc.borrow::<NSURLRequestHostObject>(this).main_document_url
}

- (NSURLRequestCachePolicy)cachePolicy {
    env.objc.borrow::<NSURLRequestHostObject>(this).cache_policy
}

- (NSTimeInterval)timeoutInterval {
    env.objc.borrow::<NSURLRequestHostObject>(this).timeout_interval
}

- (NSURLRequestNetworkServiceType)networkServiceType {
    env.objc.borrow::<NSURLRequestHostObject>(this).network_service_type
}

- (bool)allowsCellularAccess {
    env.objc.borrow::<NSURLRequestHostObject>(this).allows_cellular_access
}

- (bool)HTTPShouldHandleCookies {
    env.objc.borrow::<NSURLRequestHostObject>(this).http_should_handle_cookies
}

- (bool)HTTPShouldUsePipelining {
    env.objc.borrow::<NSURLRequestHostObject>(this).http_should_use_pipelining
}

// MARK: - HTTP accessors

- (id)HTTPMethod {
    env.objc.borrow::<NSURLRequestHostObject>(this).http_method
}

- (id)HTTPBody {
    env.objc.borrow::<NSURLRequestHostObject>(this).http_body
}

- (id)HTTPBodyStream {
    env.objc.borrow::<NSURLRequestHostObject>(this).http_body_stream
}

- (id)allHTTPHeaderFields {
    env.objc.borrow::<NSURLRequestHostObject>(this).http_header_fields
}

- (id)valueForHTTPHeaderField:(id)field {
    let fields = env.objc.borrow::<NSURLRequestHostObject>(this).http_header_fields;
    msg![env; fields objectForKey:field]
}

// MARK: - Description

- (id)description {
    let url    = env.objc.borrow::<NSURLRequestHostObject>(this).url;
    let method = env.objc.borrow::<NSURLRequestHostObject>(this).http_method;
    let method_str = if method != nil {
        to_rust_string(env, method).into_owned()
    } else {
        "GET".to_string()
    };
    let url_desc: id = if url != nil { msg![env; url description] } else { nil };
    let url_str = if url_desc != nil {
        to_rust_string(env, url_desc).into_owned()
    } else {
        "<nil>".to_string()
    };
    let s = format!("<NSURLRequest {} {}>", method_str, url_str);
    let ns = crate::frameworks::foundation::ns_string::from_rust_string(env, s);
    autorelease(env, ns)
}

@end

// MARK: - NSMutableURLRequest

@implementation NSMutableURLRequest: NSURLRequest

// MARK: URL / policy setters

- (())setURL:(id)url {
    let old = env.objc.borrow::<NSURLRequestHostObject>(this).url;
    release(env, old);
    let url_copy: id = if url != nil { msg![env; url copy] } else { nil };
    env.objc.borrow_mut::<NSURLRequestHostObject>(this).url = url_copy;
}

- (())setMainDocumentURL:(id)url {
    let old = env.objc.borrow::<NSURLRequestHostObject>(this).main_document_url;
    release(env, old);
    let url_copy: id = if url != nil { msg![env; url copy] } else { nil };
    env.objc.borrow_mut::<NSURLRequestHostObject>(this).main_document_url = url_copy;
}

- (())setCachePolicy:(NSURLRequestCachePolicy)policy {
    env.objc.borrow_mut::<NSURLRequestHostObject>(this).cache_policy = policy;
}

- (())setTimeoutInterval:(NSTimeInterval)interval {
    env.objc.borrow_mut::<NSURLRequestHostObject>(this).timeout_interval = interval;
}

- (())setNetworkServiceType:(NSURLRequestNetworkServiceType)service_type {
    env.objc.borrow_mut::<NSURLRequestHostObject>(this).network_service_type = service_type;
}

- (())setAllowsCellularAccess:(bool)allows {
    env.objc.borrow_mut::<NSURLRequestHostObject>(this).allows_cellular_access = allows;
}

- (())setHTTPShouldHandleCookies:(bool)should {
    env.objc.borrow_mut::<NSURLRequestHostObject>(this).http_should_handle_cookies = should;
}

- (())setHTTPShouldUsePipelining:(bool)should {
    env.objc.borrow_mut::<NSURLRequestHostObject>(this).http_should_use_pipelining = should;
}

// MARK: HTTP setters

- (())setHTTPMethod:(id)http_method {
    if http_method == nil { return; }
    let copy: id = msg![env; http_method copy];
    
    let (old, old_owned) = {
        let host = env.objc.borrow_mut::<NSURLRequestHostObject>(this);
        let old_owned = host.http_method_is_owned;
        let old       = std::mem::replace(&mut host.http_method, copy);
        host.http_method_is_owned = true;
        (old, old_owned)
    }; // borrow_mut drop edildi
    
    if old_owned {
        release(env, old);
    }
}

- (())setHTTPBody:(id)http_body {
    let copy: id = if http_body != nil { msg![env; http_body copy] } else { nil };
    let old = {
        std::mem::replace(
            &mut env.objc.borrow_mut::<NSURLRequestHostObject>(this).http_body,
            copy,
        )
    };
    release(env, old);
}

- (())setHTTPBodyStream:(id)stream {
    let old = env.objc.borrow::<NSURLRequestHostObject>(this).http_body_stream;
    release(env, old);
    if stream != nil { retain(env, stream); }
    env.objc.borrow_mut::<NSURLRequestHostObject>(this).http_body_stream = stream;
}

// MARK: Header fields

- (())setValue:(id)value forHTTPHeaderField:(id)field {
    if field == nil { return; }
    log_dbg!(
        "NSMutableURLRequest setValue:'{}' forHTTPHeaderField:'{}'",
        if value != nil { to_rust_string(env, value).into_owned() } else { "<nil>".into() },
        to_rust_string(env, field)
    );
    let fields = env.objc.borrow::<NSURLRequestHostObject>(this).http_header_fields;
    if value != nil {
        () = msg![env; fields setObject:value forKey:field];
    } else {
        () = msg![env; fields removeObjectForKey:field];
    }
}

- (())addValue:(id)value forHTTPHeaderField:(id)field {
    if field == nil || value == nil { return; }
    log_dbg!(
        "NSMutableURLRequest addValue:'{}' forHTTPHeaderField:'{}'",
        to_rust_string(env, value),
        to_rust_string(env, field)
    );
    let fields = env.objc.borrow::<NSURLRequestHostObject>(this).http_header_fields;
    let existing: id = msg![env; fields objectForKey:field];
    if existing != nil {
        let existing_str = to_rust_string(env, existing).into_owned();
        let new_str      = to_rust_string(env, value).into_owned();
        let combined = crate::frameworks::foundation::ns_string::from_rust_string(
            env,
            format!("{}, {}", existing_str, new_str),
        );
        autorelease(env, combined);
        () = msg![env; fields setObject:combined forKey:field];
    } else {
        () = msg![env; fields setObject:value forKey:field];
    }
}

- (())setAllHTTPHeaderFields:(id)header_fields {
    let old = env.objc.borrow::<NSURLRequestHostObject>(this).http_header_fields;
    release(env, old);
    let copy: id = if header_fields != nil {
        msg![env; header_fields mutableCopy]
    } else {
        msg_class![env; NSMutableDictionary new]
    };
    env.objc.borrow_mut::<NSURLRequestHostObject>(this).http_header_fields = copy;
}

@end

};
