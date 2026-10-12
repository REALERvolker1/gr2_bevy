using System.Security.Cryptography;
using System.Text.Json;
using UAssetAPI;
using UAssetAPI.ExportTypes;
using UAssetAPI.UnrealTypes;

if (args.Length != 2)
    throw new ArgumentException("Usage: PackageInspector INPUT.uasset NEW_OUTPUT_DIRECTORY");
string input = Path.GetFullPath(args[0]);
string output = Path.GetFullPath(args[1]);
if (Directory.Exists(output) || File.Exists(output))
    throw new IOException("Preserve existing inspection output");

var asset = new UAsset(input, EngineVersion.VER_UE4_27);
if (!asset.VerifyBinaryEquality())
    throw new InvalidDataException("Decoded asset failed in-memory binary roundtrip");

var functions = new List<FunctionIndex>();
for (int exportIndex = 0; exportIndex < asset.Exports.Count; exportIndex++)
{
    if (asset.Exports[exportIndex] is not FunctionExport function)
        continue;
    var instructions = new List<InstructionIndex>();
    uint offset = 0;
    foreach (var expression in function.ScriptBytecode)
    {
        uint size = expression.GetSize(asset);
        instructions.Add(new InstructionIndex(instructions.Count, offset, size,
            expression.Token.ToString()));
        offset = checked(offset + size);
    }
    // Jump offsets are VM offsets, not offsets in the serialized package.
    // Refuse an index with incomplete size accounting rather than guessing.
    if (offset != function.ScriptBytecodeSize)
        throw new InvalidDataException($"Bytecode size mismatch: {function.ObjectName}: {offset} != {function.ScriptBytecodeSize}");
    functions.Add(new FunctionIndex(exportIndex + 1, function.ObjectName.ToString(),
        offset, instructions));
}

Directory.CreateDirectory(output);
string assetOutput = Path.Combine(output, "asset.json");
File.WriteAllText(assetOutput, asset.SerializeJson(true));
string indexOutput = Path.Combine(output, "bytecode-index.json");
var options = new JsonSerializerOptions { WriteIndented = true };
File.WriteAllText(indexOutput, JsonSerializer.Serialize(functions, options));
var sources = new List<Artifact> { Describe(input) };
string companion = Path.ChangeExtension(input, ".uexp");
if (File.Exists(companion))
    sources.Add(Describe(companion));
File.WriteAllText(Path.Combine(output, "manifest.json"), JsonSerializer.Serialize(new
{
    engine = "UE4.27",
    binary_roundtrip = true,
    vm_sizes_checked = true,
    recorded_utc = DateTimeOffset.UtcNow,
    parser = Describe(typeof(UAsset).Assembly.Location),
    inspector = Describe(typeof(Artifact).Assembly.Location),
    sources,
    outputs = new[] { Describe(assetOutput), Describe(indexOutput) },
    exports = asset.Exports.Count,
    functions = functions.Count,
    limits = "Package decoding and VM offset indexing only; no gameplay interpretation or parity proof."
}, options));
Console.WriteLine($"Read {asset.Exports.Count} exports, indexed {functions.Count} functions; exact binary roundtrip and VM sizes checked.");

static Artifact Describe(string path)
{
    using var stream = File.OpenRead(path);
    return new Artifact(path, Convert.ToHexStringLower(SHA256.HashData(stream)), stream.Length);
}

record InstructionIndex(int ordinal, uint offset, uint size, string opcode);
record FunctionIndex(int export_index, string name, uint vm_size, List<InstructionIndex> instructions);
record Artifact(string path, string sha256, long bytes);
