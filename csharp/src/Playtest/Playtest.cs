using System.Reflection;

namespace Playtest;

/// <summary>
/// Playtest testing framework facade for .NET.
/// </summary>
public static class Playtest
{
    /// <summary>
    /// Current Playtest framework semantic version discovered from assembly metadata.
    /// </summary>
    public static string Version { get; } =
        typeof(Playtest).Assembly
            .GetCustomAttribute<AssemblyInformationalVersionAttribute>()?
            .InformationalVersion?.Split('+')[0]
        ?? throw new InvalidOperationException("Missing AssemblyInformationalVersionAttribute.");

    /// <summary>
    /// Returns the semantic version of the framework.
    /// </summary>
    public static string GetVersion() => Version;
}
