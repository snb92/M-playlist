using System.Collections.ObjectModel;
using System.IO;
using System.Text.Json;
using System.Threading.Tasks;

namespace MPlaylistApp
{
    public static class ShowFileService
    {
        private static readonly JsonSerializerOptions _options = new JsonSerializerOptions { WriteIndented = true };

        public static async Task SaveShowAsync(string path, ObservableCollection<MediaCue> cues)
        {
            using var stream = File.Create(path);
            await JsonSerializer.SerializeAsync(stream, cues, _options);
        }

        public static async Task<ObservableCollection<MediaCue>?> LoadShowAsync(string path)
        {
            using var stream = File.OpenRead(path);
            return await JsonSerializer.DeserializeAsync<ObservableCollection<MediaCue>>(stream, _options);
        }
    }
}
