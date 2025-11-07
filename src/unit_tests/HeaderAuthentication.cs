using Microsoft.AspNetCore.Authentication;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.Logging;
using Microsoft.Extensions.Options;
using System.Security.Claims;
using System.Text.Encodings.Web;

namespace unit_tests;

public static class HeaderAuthConstants
{
    public const string DefaultScheme = "HeaderAuth";
    public const string AuthorizationHeader = "Authorization";

    public const string TokenProxyPrefix = "ProxyToken-";
    public const string TokenDirectPrefix = "DirectToken-";

    public const string TokenUnknownPrefix = "UnknownToken-";
}

/// <summary>
/// Authentication scheme options for header-based authentication
/// </summary>
public class HeaderAuthenticationSchemeOptions : AuthenticationSchemeOptions
{
    /// <summary>
    /// The name of the header to check for authentication
    /// </summary>
    public string HeaderName { get; set; } = HeaderAuthConstants.AuthorizationHeader;

    public string TokenPrefix { get; set; } = HeaderAuthConstants.TokenUnknownPrefix;
}

/// <summary>
/// Authentication handler that validates requests based on the presence of a specific header
/// </summary>
public class HeaderAuthenticationHandler : AuthenticationHandler<HeaderAuthenticationSchemeOptions>
{
    public HeaderAuthenticationHandler(IOptionsMonitor<HeaderAuthenticationSchemeOptions> options,
        ILoggerFactory logger, UrlEncoder encoder)
        : base(options, logger, encoder)
    {
    }

    protected override Task<AuthenticateResult> HandleAuthenticateAsync()
    {
        // Check if the authorization header exists
        if (!Request.Headers.ContainsKey(Options.HeaderName))
        {
            Logger.LogWarning("Missing {HeaderName} header", Options.HeaderName);
            return Task.FromResult(AuthenticateResult.Fail($"Missing {Options.HeaderName} header"));
        }

        var headerValue = Request.Headers[Options.HeaderName].FirstOrDefault();

        if (string.IsNullOrEmpty(headerValue))
        {
            Logger.LogWarning("Empty {HeaderName} header", Options.HeaderName);
            return Task.FromResult(AuthenticateResult.Fail($"Empty {Options.HeaderName} header"));
        }

        // Check the header has the expected token prefix
        if (!headerValue.StartsWith(Options.TokenPrefix))
        {
            Logger.LogWarning("Invalid token prefix in {HeaderName} header: {HeaderValue}, want {Prefix}", Options.HeaderName, headerValue, Options.TokenPrefix);
            return Task.FromResult(AuthenticateResult.Fail($"Invalid token prefix in {Options.HeaderName} header"));
        }

        // Create claims and identity
        var claims = new[]
        {
            new Claim(ClaimTypes.Name, "AuthenticatedUser"),
            new Claim("token", headerValue)
        };

        var identity = new ClaimsIdentity(claims, Scheme.Name);
        var principal = new ClaimsPrincipal(identity);
        var ticket = new AuthenticationTicket(principal, Scheme.Name);

        return Task.FromResult(AuthenticateResult.Success(ticket));
    }
}

/// <summary>
/// Extension methods for configuring header authentication
/// </summary>
public static class HeaderAuthenticationExtensions
{
    /// <summary>
    /// Adds header authentication to the service collection
    /// </summary>
    /// <param name="services">The service collection</param>
    /// <param name="schemeName">The authentication scheme name (default: "HeaderAuth")</param>
    /// <param name="headerName">The header name to check (default: "Authorization")</param>
    /// <returns>The authentication builder</returns>
    public static AuthenticationBuilder AddHeaderAuthentication(
        this IServiceCollection services,
        string tokenPrefix,
        string schemeName = HeaderAuthConstants.DefaultScheme,
        string headerName = HeaderAuthConstants.AuthorizationHeader)
    {
        return services.AddAuthentication(schemeName)
            .AddScheme<HeaderAuthenticationSchemeOptions, HeaderAuthenticationHandler>(
                schemeName, options =>
                {
                    options.HeaderName = headerName;
                    options.TokenPrefix = tokenPrefix;
                });
    }
}